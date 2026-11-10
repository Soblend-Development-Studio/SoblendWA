use std::sync::atomic::Ordering;
use std::time::Instant;

use prost::Message as _;

use super::polls::has_addon_secret;
use super::prekeys::{pre_key_node, signed_pre_key_node};
use super::Client;
use crate::binary::Node;
use crate::crypto::KEY_TYPE;
use crate::error::{Error, Result};
use crate::events::{Event, MessageEvent, MessageInfo};
use crate::jid::Jid;
use crate::message::{now_secs, unpad};
use crate::proto::{Message, ProtocolMessage};
use crate::signal::EncType;

const MAX_RETRY_REQUESTS: u32 = 5;

impl Client {
    pub(super) async fn handle_message(&self, node: Node) {
        let received_at = Instant::now();
        let mut info = match self.parse_message_info(&node, received_at) {
            Ok(info) => info,
            Err(err) => {
                tracing::warn!(%err, "ignoring malformed message stanza");
                self.send_ack(&node, None);
                return;
            }
        };
        if node.child("unavailable").is_some() && node.child("enc").is_none() {
            self.send_ack(&node, None);
            return;
        }
        self.resolve_sender(&mut info).await;

        let mut combined: Option<Message> = None;
        let mut failure: Option<Error> = None;
        for enc in node.nodes().iter().filter(|n| n.tag == "enc" || n.tag == "plaintext") {
            let Some(data) = enc.content_bytes() else { continue };
            match self.decrypt_payload(&info, enc, data).await {
                Ok(message) => match combined.as_mut() {
                    Some(existing) => {
                        if let Err(err) = existing.merge(message.encode_to_vec().as_slice()) {
                            failure = Some(err.into());
                        }
                    }
                    None => combined = Some(message),
                },
                Err(err) => {
                    tracing::warn!(id = %info.id, sender = %info.sender, %err, "failed to decrypt message");
                    failure = Some(err);
                }
            }
        }

        match (combined, failure) {
            (Some(message), None) | (Some(message), Some(_)) if message.has_content() => {
                self.acknowledge_delivery(&node, &info, &message);
                let key = format!("{}|{}|{}", info.chat, info.sender.to_non_device(), info.id);
                if !self.inner.seen.lock().first_time(key) {
                    super::guard::Counters::bump(&self.inner.counters.duplicates);
                    tracing::debug!(id = %info.id, "dropped a duplicate of a message we already delivered");
                    self.forget_retry(&info.id);
                    self.send_ack(&node, None);
                    return;
                }
                if message.protocol_message.is_none() {
                    self.remember_last_message(&info.chat, info.key(), info.timestamp);
                }
                let unwrapped = message.unwrapped().clone();
                let event = MessageEvent { info, message: unwrapped, raw: message };
                if self.side_effects_of(&event) {
                    self.emit(Event::Message(Box::new(event.clone())));
                    self.process_side_effects(&event).await;
                } else {
                    self.emit(Event::Message(Box::new(event)));
                }
            }
            (Some(message), None) => self.acknowledge_delivery(&node, &info, &message),
            (_, Some(err)) => {
                self.await_retry(&info, &err.to_string()).await;
                if let Err(retry_err) = self.request_retry(&node, &info).await {
                    tracing::debug!(%retry_err, "could not send retry receipt");
                }
                self.emit(Event::UndecryptableMessage { info: Box::new(info), error: err.to_string() });
            }
            (None, None) => {}
        }
        self.send_ack(&node, None);
    }

    fn side_effects_of(&self, event: &MessageEvent) -> bool {
        let m = &event.message;
        has_addon_secret(m)
            || m.poll_update_message.is_some()
            || m.enc_event_response_message.is_some()
            || m.protocol_message.as_ref().is_some_and(|p| {
                p.history_sync_notification.is_some()
                    || p.app_state_sync_key_share.is_some()
                    || p.peer_data_operation_request_response_message.is_some()
            })
    }

    async fn process_side_effects(&self, event: &MessageEvent) {
        let (info, message) = (&event.info, &event.message);
        if has_addon_secret(message) {
            self.remember_incoming_poll(info, &event.raw).await;
        }
        if let Some(response) = &message.enc_event_response_message {
            if let Some(answer) = self.decode_event_response(info, response).await {
                self.emit(Event::EventResponse(answer));
            }
        }
        if let Some(update) = &message.poll_update_message {
            if let Some(vote) = self.decode_poll_vote(info, update).await {
                self.emit(Event::PollVote(vote));
            }
        }
        if let Some(notification) = message.protocol_message.as_ref().and_then(|p| p.history_sync_notification.clone())
        {
            if info.from_me {
                self.process_history_sync(notification);
            }
        }
        if let Some(response) =
            message.protocol_message.as_ref().and_then(|p| p.peer_data_operation_request_response_message.as_ref())
        {
            if info.from_me {
                self.process_peer_response(response);
            }
        }
        if let Some(share) = message.protocol_message.as_ref().and_then(|p| p.app_state_sync_key_share.as_ref()) {
            if info.from_me {
                match self.store_app_state_keys(share).await {
                    Ok(first) => {
                        let all = super::appstate::COLLECTIONS.iter().map(|c| (*c).to_owned()).collect();
                        self.sync_app_state_later(all, first);
                    }
                    Err(err) => tracing::warn!(%err, "could not store app state keys"),
                }
            }
        }
    }

    pub(crate) async fn resolve_sender(&self, info: &mut MessageInfo) {
        if info.from_me {
            info.sender_phone = self.me().map(|j| j.to_non_device());
            info.sender_lid = self.lid().map(|j| j.to_non_device());
            return;
        }
        let sender = info.sender.to_non_device();
        let alt = info.sender_alt.as_ref().map(Jid::to_non_device);
        let (mut phone, mut lid) = if sender.is_lid() {
            (alt.filter(Jid::is_pn), Some(sender.clone()))
        } else if sender.is_pn() {
            (Some(sender.clone()), alt.filter(Jid::is_lid))
        } else {
            (None, None)
        };
        if phone.is_none() || lid.is_none() {
            if let Some(member) = self.inner.groups.get(&info.chat).and_then(|g| g.participant(&sender).cloned()) {
                phone = phone.or(member.phone_number.map(|j| j.to_non_device()));
                lid = lid.or(member.lid.map(|j| j.to_non_device()));
                if member.jid.is_lid() {
                    lid = lid.or(Some(member.jid.to_non_device()));
                } else {
                    phone = phone.or(Some(member.jid.to_non_device()));
                }
            }
        }
        if let (None, Some(known)) = (&phone, &lid) {
            phone = self.phone_for_lid(known).await.ok().flatten();
        }
        if let (Some(known), None) = (&phone, &lid) {
            lid = self.lid_for_phone(known).await.ok().flatten();
        }
        info.sender_phone = phone;
        info.sender_lid = lid;
    }

    fn parse_message_info(&self, node: &Node, received_at: Instant) -> Result<MessageInfo> {
        let from = node.attr_jid("from").ok_or_else(|| Error::binary("message without sender"))?;
        let id = node.attr_string("id").ok_or_else(|| Error::binary("message without id"))?;
        let participant = node.attr_jid("participant");

        let (chat, sender) = if from.is_group() || from.is_broadcast() {
            let sender = participant.ok_or_else(|| Error::binary("group message without participant"))?;
            (from, sender)
        } else if from.is_user() {
            match node.attr_jid("recipient") {
                Some(recipient) if self.is_me(&from) => (recipient.to_non_device(), from),
                _ => (from.to_non_device(), from),
            }
        } else if from.is_newsletter() {
            (from.clone(), from)
        } else {
            return Err(Error::binary(format!("unsupported message source {from}")));
        };

        let sender_alt =
            ["participant_pn", "sender_pn", "participant_lid", "sender_lid"].iter().find_map(|key| node.attr_jid(key));
        if let Some(alt) = &sender_alt {
            let (pn, lid) = if sender.is_lid() { (alt, &sender) } else { (&sender, alt) };
            let client = self.clone();
            let (pn, lid) = (pn.to_non_device(), lid.to_non_device());
            tokio::spawn(async move { client.remember_lid(&pn, &lid).await });
        }

        Ok(MessageInfo {
            is_group: chat.is_group(),
            from_me: self.is_me(&sender),
            push_name: node.attr_string("notify"),
            timestamp: node.attr_i64("t").unwrap_or_else(now_secs),
            category: node.attr_string("category"),
            offline: node.has("offline"),
            edit: node.attr_string("edit"),
            server_id: node.attr_string("server_id"),
            id,
            chat,
            sender,
            sender_alt,
            sender_phone: None,
            sender_lid: None,
            received_at,
        })
    }

    async fn decrypt_payload(&self, info: &MessageInfo, enc: &Node, data: &[u8]) -> Result<Message> {
        let signal = self.signal();
        let kind = if enc.tag == "plaintext" { "plaintext" } else { enc.attr_str("type").unwrap_or_default() };
        let plaintext = match kind {
            "plaintext" => return Ok(Message::decode(data)?),
            "skmsg" => signal.group_decrypt(&info.chat.to_string(), &info.sender.signal_address(), data).await?,
            "pkmsg" => signal.decrypt(&info.sender.signal_address(), EncType::PreKey, data).await?,
            "msg" => signal.decrypt(&info.sender.signal_address(), EncType::Message, data).await?,
            other => return Err(Error::signal(format!("unknown encryption type {other}"))),
        };
        let body = unpad(&plaintext).ok_or_else(|| Error::signal("invalid message padding"))?;
        let mut message = Message::decode(body)?;
        if !info.from_me {
            self.learn_device(&info.sender).await;
        }
        // what we send from the phone reaches our other devices wrapped in deviceSentMessage
        if let Some(sent) = message.device_sent_message.take() {
            if let Some(inner) = sent.message {
                message = *inner;
            }
        }
        if let Some(distribution) = &message.sender_key_distribution_message {
            let group = distribution.group_id.clone().unwrap_or_else(|| info.chat.to_string());
            if let Some(raw) = &distribution.axolotl_sender_key_distribution_message {
                if let Err(err) = signal.process_distribution(&group, &info.sender.signal_address(), raw).await {
                    tracing::warn!(%err, "invalid sender key distribution");
                }
            }
        }
        Ok(message)
    }

    fn acknowledge_delivery(&self, node: &Node, info: &MessageInfo, message: &Message) {
        let is_history = message
            .protocol_message
            .as_ref()
            .is_some_and(|p| p.r#type == Some(ProtocolMessage::HISTORY_SYNC_NOTIFICATION));
        let kind = if is_history {
            Some("hist_sync")
        } else if info.category.as_deref() == Some("peer") {
            Some("peer_msg")
        } else if info.from_me {
            Some("sender")
        } else if !self.inner.active.load(Ordering::Acquire) {
            Some("inactive")
        } else {
            None
        };

        let mut receipt = Node::new("receipt").attr("id", info.id.clone());
        if kind == Some("sender") && !info.is_group {
            receipt.set_attr("to", info.sender.clone());
            receipt.set_attr("recipient", info.chat.clone());
        } else {
            if let Some(from) = node.get("from") {
                receipt.set_attr("to", from.clone());
            }
            if let Some(participant) = node.get("participant") {
                receipt.set_attr("participant", participant.clone());
            }
        }
        if let Some(kind) = kind {
            receipt.set_attr("type", kind);
        }
        if let Err(err) = self.send_node(&receipt) {
            tracing::debug!(%err, "could not send delivery receipt");
        }
    }

    async fn request_retry(&self, node: &Node, info: &MessageInfo) -> Result<()> {
        let count = self.bump_retry(&format!("{}:{}", info.id, info.sender));
        if count > MAX_RETRY_REQUESTS {
            return Ok(());
        }
        if count == 1 && !info.from_me && info.category.as_deref() != Some("peer") {
            let client = self.clone();
            let key = info.key();
            tokio::spawn(async move {
                if let Err(err) = client.request_placeholder_resend(vec![key]).await {
                    tracing::debug!(%err, "could not ask the phone to resend a message");
                }
            });
        }
        let creds = self.creds();
        let mut receipt = Node::new("receipt").attr("id", info.id.clone()).attr("type", "retry");
        for key in ["from", "participant", "recipient"] {
            if let Some(value) = node.get(key) {
                receipt.set_attr(if key == "from" { "to" } else { key }, value.clone());
            }
        }
        let mut retry = Node::new("retry").attr("count", count).attr("id", info.id.clone()).attr("v", "1");
        if let Some(t) = node.get("t") {
            retry.set_attr("t", t.clone());
        }
        let mut children = vec![retry, Node::new("registration").bytes(creds.registration_id.to_be_bytes().to_vec())];
        if count > 1 {
            let (id, pair) =
                self.generate_pre_keys(1).await?.pop().ok_or_else(|| Error::signal("no pre-key generated"))?;
            let mut keys = vec![
                Node::new("type").bytes(vec![KEY_TYPE]),
                Node::new("identity").bytes(creds.identity_key.public.to_vec()),
                pre_key_node(id, &pair),
                signed_pre_key_node(&creds),
            ];
            if let Some(account) = &creds.account {
                keys.push(Node::new("device-identity").bytes(account.clone()));
            }
            children.push(Node::new("keys").children(keys));
        }
        self.send_node(&receipt.children(children))
    }

    pub async fn mark_read(&self, chat: &Jid, sender: Option<&Jid>, ids: &[String]) -> Result<()> {
        self.send_receipt(chat, sender, ids, "read").await
    }

    pub async fn mark_played(&self, event: &MessageEvent) -> Result<()> {
        let sender = event.info.is_group.then_some(&event.info.sender);
        self.send_receipt(&event.info.chat, sender, std::slice::from_ref(&event.info.id), "played").await
    }

    pub async fn send_receipt(&self, chat: &Jid, sender: Option<&Jid>, ids: &[String], kind: &str) -> Result<()> {
        let Some((first, rest)) = ids.split_first() else { return Ok(()) };
        let mut receipt = Node::new("receipt").attr("id", first.clone()).attr("to", chat);
        if !kind.is_empty() {
            receipt.set_attr("type", kind.to_owned());
        }
        if matches!(kind, "read" | "read-self") {
            receipt.set_attr("t", now_secs());
        }
        if let Some(sender) = sender.filter(|_| chat.is_group()) {
            receipt.set_attr("participant", sender);
        }
        if !rest.is_empty() {
            receipt
                .push_child(Node::new("list").children(rest.iter().map(|id| Node::new("item").attr("id", id.clone()))));
        }
        self.send_node(&receipt)
    }

    pub async fn read_message(&self, event: &MessageEvent) -> Result<()> {
        let sender = event.info.is_group.then_some(&event.info.sender);
        self.mark_read(&event.info.chat, sender, std::slice::from_ref(&event.info.id)).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::client::GroupInfo;

    fn info(chat: Jid, sender: Jid, sender_alt: Option<Jid>) -> MessageInfo {
        MessageInfo {
            id: "3EB0".into(),
            is_group: chat.is_group(),
            chat,
            sender,
            sender_alt,
            sender_phone: None,
            sender_lid: None,
            from_me: false,
            push_name: None,
            timestamp: 0,
            category: None,
            offline: false,
            edit: None,
            server_id: None,
            received_at: Instant::now(),
        }
    }

    #[tokio::test]
    async fn phone_and_lid() {
        let (client, _events) = Client::builder().build().await.unwrap();
        let phone = Jid::phone("5215511111111");
        let lid = Jid::lid("100000000000001");

        let mut direct = info(phone.clone(), phone.with_device(3), Some(lid.clone()));
        client.resolve_sender(&mut direct).await;
        assert_eq!((direct.sender_phone.as_ref(), direct.sender_lid.as_ref()), (Some(&phone), Some(&lid)));

        let group = Jid::group("120363000000000001");
        let node = Node::new("group").attr("id", "120363000000000001").attr("subject", "Equipo").children([Node::new(
            "participant",
        )
        .attr("jid", lid.clone())
        .attr("phone_number", phone.clone())
        .attr("type", "admin")]);
        client.inner.groups.insert(group.clone(), GroupInfo::parse(&node).unwrap());
        let mut in_group = info(group.clone(), lid.with_device(2), None);
        client.resolve_sender(&mut in_group).await;
        assert_eq!(in_group.sender_phone.as_ref(), Some(&phone));
        assert!(client.inner.groups.get(&group).unwrap().is_admin(&phone));

        client.remember_lid(&phone, &lid).await;
        let mut by_mapping = info(lid.clone(), lid.clone(), None);
        client.resolve_sender(&mut by_mapping).await;
        assert_eq!(by_mapping.sender_phone.as_ref(), Some(&phone));

        let stale = MessageInfo { offline: true, ..info(phone.clone(), phone.clone(), None) };
        assert!(stale.is_stale(std::time::Duration::from_secs(60)));
    }
}
