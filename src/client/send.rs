use std::collections::HashSet;
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use prost::Message as _;

use super::guard::{Counters, SendTimings};
use super::polls::{event_of, has_addon_secret, poll_of};
use super::reporting::{reporting_node, wants_reporting_token};
use super::{Client, SentMessage};
use crate::binary::Node;
use crate::crypto::{self, strip_key_prefix};
use crate::error::{Error, Result};
use crate::events::MessageEvent;
use crate::jid::Jid;
use crate::message::{generate_message_id, pad};
use crate::proto::{
    AlbumMessage, DeviceSentMessage, Message, MessageAssociation, MessageKey, SenderKeyDistributionMessage,
};
use crate::signal::{EncType, PreKeyBundle};
use crate::store::KeyKind;

const MAX_RESENDS: u32 = 5;
const KEY_BATCH: usize = 256;

#[derive(Clone, Debug, Default)]
pub struct SendOptions {
    pub id: Option<String>,
    pub edit: Option<&'static str>,
    pub status_recipients: Vec<Jid>,
    pub link_preview: super::PreviewChoice,
}

#[derive(Clone, Copy, Default)]
struct EncAttrs {
    media_type: Option<&'static str>,
    hide_failures: bool,
}

impl EncAttrs {
    fn of(message: &Message) -> Self {
        let inner = message.unwrapped();
        EncAttrs {
            media_type: message.media_type(),
            hide_failures: inner.reaction_message.is_some() || inner.pin_in_chat_message.is_some(),
        }
    }
}

fn enc_node(kind: &'static str, ciphertext: Vec<u8>, attrs: EncAttrs) -> Node {
    let node = Node::new("enc").attr("v", "2").attr("type", kind).attr_opt("mediatype", attrs.media_type);
    let node = if attrs.hide_failures { node.attr("decrypt-fail", "hide") } else { node };
    node.bytes(ciphertext)
}

fn parse_bundle(container: &Node, registration_id: Option<u32>) -> Option<PreKeyBundle> {
    let identity = strip_key_prefix(container.child_bytes("identity")?).ok()?;
    let signed = container.child("skey")?;
    let one_time = container.child("key").and_then(|key| {
        let id = key.child_uint("id")?;
        let value = strip_key_prefix(key.child_bytes("value")?).ok()?;
        Some((id, value))
    });
    Some(PreKeyBundle {
        registration_id: registration_id.or_else(|| container.child_uint("registration"))?,
        identity_key: identity,
        signed_pre_key_id: signed.child_uint("id")?,
        signed_pre_key: strip_key_prefix(signed.child_bytes("value")?).ok()?,
        signed_pre_key_signature: signed.child_bytes("signature")?.try_into().ok()?,
        pre_key: one_time,
    })
}

impl Client {
    pub async fn send_message(&self, chat: &Jid, message: impl Into<Message>) -> Result<String> {
        self.send_message_with(chat, message.into(), SendOptions::default()).await
    }

    pub async fn send_text(&self, chat: &Jid, text: impl Into<String>) -> Result<String> {
        self.send_message(chat, Message::text(text)).await
    }

    pub async fn reply(&self, to: &MessageEvent, message: impl Into<Message>) -> Result<String> {
        self.send_message(&to.info.chat, message.into().reply_to(to)).await
    }

    pub async fn react(&self, to: &MessageEvent, emoji: &str) -> Result<String> {
        self.send_message(&to.info.chat, Message::reaction(to.key(), emoji)).await
    }

    pub async fn edit_message(&self, chat: &Jid, id: &str, replacement: impl Into<Message>) -> Result<String> {
        let key = MessageKey {
            remote_jid: Some(chat.to_string()),
            from_me: Some(true),
            id: Some(id.to_owned()),
            participant: None,
        };
        let options = SendOptions { edit: Some("1"), ..Default::default() };
        self.send_message_with(chat, Message::edit(key, replacement.into()), options).await
    }

    pub async fn revoke_message(&self, chat: &Jid, key: MessageKey) -> Result<String> {
        let admin_revoke = chat.is_group() && key.from_me != Some(true);
        let options = SendOptions { edit: Some(if admin_revoke { "8" } else { "7" }), ..Default::default() };
        self.send_message_with(chat, Message::revoke(key), options).await
    }

    pub async fn pin_message(&self, chat: &Jid, key: MessageKey, duration_secs: u32) -> Result<String> {
        self.send_message(chat, Message::pin(key, true, duration_secs)).await
    }

    pub async fn unpin_message(&self, chat: &Jid, key: MessageKey) -> Result<String> {
        self.send_message(chat, Message::pin(key, false, 0)).await
    }

    pub async fn send_album(&self, chat: &Jid, items: Vec<Message>) -> Result<Vec<String>> {
        let count = |pick: fn(&Message) -> bool| items.iter().filter(|m| pick(m)).count() as u32;
        let album = AlbumMessage {
            expected_image_count: Some(count(|m| m.image_message.is_some())),
            expected_video_count: Some(count(|m| m.video_message.is_some())),
            context_info: None,
        };
        let album_id =
            self.send_message(chat, Message { album_message: Some(Box::new(album)), ..Default::default() }).await?;
        let parent = MessageKey {
            remote_jid: Some(chat.to_non_device().to_string()),
            from_me: Some(true),
            id: Some(album_id.clone()),
            participant: None,
        };
        let mut ids = vec![album_id];
        for mut item in items {
            item.message_context_info.get_or_insert_with(Default::default).message_association =
                Some(MessageAssociation {
                    association_type: Some(MessageAssociation::MEDIA_ALBUM),
                    parent_message_key: Some(parent.clone()),
                    message_index: None,
                });
            ids.push(self.send_message(chat, item).await?);
        }
        Ok(ids)
    }

    pub async fn send_status(&self, message: impl Into<Message>, recipients: &[Jid]) -> Result<String> {
        let options = SendOptions { status_recipients: recipients.to_vec(), ..Default::default() };
        self.send_message_with(&Jid::status_broadcast(), message.into(), options).await
    }

    pub async fn send_message_with(&self, chat: &Jid, message: Message, options: SendOptions) -> Result<String> {
        let chat = chat.to_non_device();
        self.admit_send(&chat)?;
        self.send_in_lane(&chat, Box::pin(self.deliver(&chat, message, options, false))).await
    }

    pub(crate) async fn resend(&self, chat: &Jid, message: Message, options: SendOptions) -> Result<String> {
        let chat = chat.to_non_device();
        self.send_in_lane(&chat, Box::pin(self.deliver(&chat, message, options, true))).await
    }

    pub(crate) async fn send_in_lane(
        &self,
        chat: &Jid,
        work: impl std::future::Future<Output = Result<String>>,
    ) -> Result<String> {
        let (result, queued) = self.in_chat_lane(chat, work).await;
        if let Ok(id) = &result {
            if !queued.is_zero() {
                self.inner.sent.lock().note_queued(id, queued);
            }
        }
        result
    }

    pub(super) async fn deliver(
        &self,
        chat: &Jid,
        mut message: Message,
        options: SendOptions,
        resend: bool,
    ) -> Result<String> {
        let me = self.require_me()?;
        let lid = self.lid();
        let id = options.id.unwrap_or_else(|| generate_message_id(Some(&me)));
        let chat = chat.to_non_device();
        let parks = !resend && self.outbox_enabled(&chat);
        if parks && !self.is_connected() && self.inner.running.load(std::sync::atomic::Ordering::Acquire) {
            self.park(&id, &chat, &message, options.edit).await;
            tracing::debug!(%id, %chat, "offline: the message waits in the outbox until the session reconnects");
            return Ok(id);
        }
        let started = Instant::now();
        let mut encrypting = Duration::ZERO;
        if !resend {
            message = self.apply_link_preview(&chat, message, &options.link_preview).await;
        }
        let reportable = !chat.is_newsletter() && wants_reporting_token(&message);
        if reportable {
            let context = message.message_context_info.get_or_insert_with(Default::default);
            context.message_secret.get_or_insert_with(|| crypto::random_vec(32));
        }
        let attrs = EncAttrs::of(&message);
        let encoded = message.encode_to_vec();

        let mut stanza =
            Node::new("message").attr("id", id.clone()).attr("to", &chat).attr("type", message.stanza_type());
        let pinning = message.unwrapped().pin_in_chat_message.is_some();
        if let Some(edit) = options.edit.or(pinning.then_some("2")) {
            stanza.set_attr("edit", edit);
        }

        let mut content = Vec::new();
        let mut include_identity = false;
        let mut sender_address = None;

        if chat.is_newsletter() {
            content.push(Node::new("plaintext").bytes(encoded.clone()));
        } else if chat.is_group() || chat.is_status_broadcast() {
            let (members, lid_addressed) = if chat.is_group() {
                let group = self.group_info(&chat).await?;
                stanza.set_attr("addressing_mode", if group.is_lid_addressed() { "lid" } else { "pn" });
                if group.ephemeral > 0 {
                    stanza.set_attr("expiration", group.ephemeral);
                }
                (group.participants.iter().map(|p| p.jid.clone()).collect::<Vec<_>>(), group.is_lid_addressed())
            } else {
                if options.status_recipients.is_empty() {
                    return Err(Error::other("a status needs at least one recipient"));
                }
                (options.status_recipients.clone(), false)
            };
            let own = match (&lid, lid_addressed) {
                (Some(lid), true) => lid.with_device(me.device),
                _ => me.clone(),
            };
            let address = own.signal_address();
            let devices = self.devices_for(&members).await?;
            let plaintext = pad(encoded.clone());
            let clock = Instant::now();
            let (skmsg, distribution) = self.signal().group_encrypt(&chat.to_string(), &address, &plaintext).await?;
            encrypting += clock.elapsed();

            let memory = if chat.is_group() { self.sender_memory(&chat).await } else { HashSet::new() };
            let targets: Vec<Jid> = devices.into_iter().filter(|d| !memory.contains(&d.to_string())).collect();
            if !targets.is_empty() {
                let skdm = Message {
                    sender_key_distribution_message: Some(Box::new(SenderKeyDistributionMessage {
                        group_id: Some(chat.to_string()),
                        axolotl_sender_key_distribution_message: Some(distribution),
                    })),
                    ..Default::default()
                };
                self.ensure_sessions(&targets).await?;
                let clock = Instant::now();
                let (nodes, delivered, pkmsg) = self.encrypt_for(&targets, &pad(skdm.encode_to_vec()), attrs).await;
                encrypting += clock.elapsed();
                include_identity |= pkmsg;
                if !nodes.is_empty() {
                    content.push(Node::new("participants").children(nodes));
                }
                if chat.is_group() {
                    self.remember_sender_key_targets(&chat, delivered).await;
                }
            }
            content.push(enc_node("skmsg", skmsg, attrs));
            sender_address = Some(address);
        } else {
            let own = match (&lid, chat.is_lid()) {
                (Some(lid), true) => lid.with_device(me.device),
                _ => me.clone(),
            };
            let devices = self.devices_for(&[chat.clone(), own.to_non_device()]).await?;
            if devices.is_empty() {
                return Err(Error::NoDevices(chat.clone()));
            }
            self.ensure_sessions(&devices).await?;
            let (mine, theirs): (Vec<Jid>, Vec<Jid>) = devices.into_iter().partition(|d| d.same_user(&own));
            let device_sent = Message {
                device_sent_message: Some(Box::new(DeviceSentMessage {
                    destination_jid: Some(chat.to_string()),
                    message: Some(Box::new(message.clone())),
                    phash: None,
                })),
                message_context_info: message.message_context_info.clone(),
                ..Default::default()
            };
            let clock = Instant::now();
            let (mut nodes, _, pkmsg_theirs) = self.encrypt_for(&theirs, &pad(encoded.clone()), attrs).await;
            let (own_nodes, _, pkmsg_mine) = self.encrypt_for(&mine, &pad(device_sent.encode_to_vec()), attrs).await;
            encrypting += clock.elapsed();
            nodes.extend(own_nodes);
            if nodes.is_empty() {
                return Err(Error::signal(format!("could not encrypt for any device of {chat}")));
            }
            include_identity = pkmsg_theirs || pkmsg_mine;
            content.push(Node::new("participants").children(nodes));
        }

        if include_identity {
            if let Some(account) = self.creds().account {
                content.push(Node::new("device-identity").bytes(account));
            }
        }
        let secret = message.message_context_info.as_ref().and_then(|c| c.message_secret.as_deref());
        if let Some(secret) = secret.filter(|_| reportable) {
            content.extend(reporting_node(&encoded, secret, &id, &chat.to_string()));
        }
        let direct = chat.is_user() && message.protocol_message.is_none();
        if direct {
            content.extend(self.direct_tc_token_node(&chat).await);
        }
        let is_poll = poll_of(&message).is_some();
        if is_poll {
            content.push(Node::new("meta").attr("polltype", "creation"));
        } else if event_of(&message).is_some() {
            content.push(Node::new("meta").attr("event_type", "creation"));
        }
        let member_label = message.protocol_message.as_ref().and_then(|p| p.r#type)
            == Some(crate::proto::ProtocolMessage::GROUP_MEMBER_LABEL_CHANGE);
        if member_label {
            content.push(Node::new("meta").attr("tag_reason", "user_update").attr("appdata", "member_tag"));
        }
        if let Some(biz) = crate::interactive::biz_node(&message) {
            content.push(biz);
        }
        let stanza = stanza.children(content);
        if parks {
            self.park(&id, &chat, &message, options.edit).await;
        }
        let timings = SendTimings {
            queued: Duration::ZERO,
            upload: self.inner.upload_times.lock().remove(&id),
            prepare: started.elapsed().saturating_sub(encrypting),
            encrypt: encrypting,
            server: None,
        };
        let sent_at = Instant::now();
        self.inner.in_flight.insert(id.clone());
        if let Err(err) = self.send_node(&stanza) {
            self.inner.in_flight.remove(&id);
            if !self.outbox_enabled(&chat) {
                return Err(err);
            }
            tracing::debug!(%err, %id, "the socket dropped; the message stays in the outbox for the reconnect");
            return Ok(id);
        }
        Counters::bump(&self.inner.counters.sent);
        if direct {
            self.issue_tc_token_later(chat.clone());
        }
        if message.protocol_message.is_none() && message.reaction_message.is_none() {
            let key = MessageKey {
                remote_jid: Some(chat.to_string()),
                from_me: Some(true),
                id: Some(id.clone()),
                participant: None,
            };
            self.remember_last_message(&chat, key, crate::message::now_secs());
        }
        if has_addon_secret(&message) {
            self.remember_poll(&id, &chat, &message, self.own_identities()).await;
        }
        if let Some(chats) = &self.inner.chat_store {
            chats.record_sent(&chat, &id, Some(me.to_non_device()), &message);
        }
        if self.outbox_enabled(&chat) {
            self.keep_for_retries(&id, &chat, &message, sender_address.as_deref()).await;
        }
        self.remember_sent(&id, SentMessage { chat, message, sender_address, sent_at, timings });
        Ok(id)
    }

    async fn encrypt_for(&self, devices: &[Jid], plaintext: &[u8], attrs: EncAttrs) -> (Vec<Node>, Vec<String>, bool) {
        if devices.is_empty() {
            return (Vec::new(), Vec::new(), false);
        }
        let addresses: Vec<String> = devices.iter().map(Jid::signal_address).collect();
        let results = self.signal().encrypt_many(&addresses, plaintext).await;
        let mut nodes = Vec::with_capacity(devices.len());
        let mut delivered = Vec::with_capacity(devices.len());
        let mut has_pre_key = false;
        for (device, (_, result)) in devices.iter().zip(results) {
            match result {
                Ok((kind, ciphertext)) => {
                    has_pre_key |= kind == EncType::PreKey;
                    delivered.push(device.to_string());
                    nodes.push(Node::new("to").attr("jid", device).child_node(enc_node(
                        kind.as_str(),
                        ciphertext,
                        attrs,
                    )));
                }
                Err(err) => tracing::warn!(%device, %err, "skipping device that failed to encrypt"),
            }
        }
        (nodes, delivered, has_pre_key)
    }

    pub(crate) async fn ensure_sessions(&self, devices: &[Jid]) -> Result<()> {
        let addresses: Vec<String> = devices.iter().map(Jid::signal_address).collect();
        let missing: HashSet<String> = self.signal().missing_sessions(&addresses).await?.into_iter().collect();
        if missing.is_empty() {
            return Ok(());
        }
        let targets: Vec<Jid> = devices.iter().filter(|d| missing.contains(&d.signal_address())).cloned().collect();
        self.fetch_sessions(&targets, false).await
    }

    pub(crate) async fn fetch_sessions(&self, devices: &[Jid], identity_refresh: bool) -> Result<()> {
        let requests = devices.chunks(KEY_BATCH).map(|chunk| {
            let users = chunk.iter().map(|jid| {
                let node = Node::new("user").attr("jid", jid);
                if identity_refresh {
                    node.attr("reason", "identity")
                } else {
                    node
                }
            });
            self.query(
                Node::new("iq")
                    .attr("to", Jid::server())
                    .attr("type", "get")
                    .attr("xmlns", "encrypt")
                    .child_node(Node::new("key").children(users)),
            )
        });

        let mut bundles = Vec::with_capacity(devices.len());
        for response in join_all(requests).await {
            let response = response?;
            for user in response.child("list").map(Node::nodes).unwrap_or_default() {
                let Some(jid) = user.attr_jid("jid") else { continue };
                if let Some((code, text)) = user.error_code() {
                    tracing::warn!(%jid, code, %text, "server refused a pre-key bundle");
                    continue;
                }
                match parse_bundle(user, None) {
                    Some(bundle) => bundles.push((jid.signal_address(), bundle)),
                    None => tracing::warn!(%jid, "incomplete pre-key bundle"),
                }
            }
        }
        for (address, result) in self.signal().inject_bundles(bundles).await {
            if let Err(err) = result {
                tracing::warn!(%address, %err, "invalid pre-key bundle");
            }
        }
        Ok(())
    }

    async fn sender_memory(&self, group: &Jid) -> HashSet<String> {
        if let Some(memory) = self.inner.sender_memory.get(group) {
            return memory.clone();
        }
        let loaded: HashSet<String> = match self.inner.store.get(KeyKind::SenderKeyMemory, &group.to_string()).await {
            Ok(Some(raw)) => String::from_utf8_lossy(&raw).lines().map(str::to_owned).collect(),
            _ => HashSet::new(),
        };
        self.inner.sender_memory.insert(group.clone(), loaded.clone());
        loaded
    }

    async fn remember_sender_key_targets(&self, group: &Jid, delivered: Vec<String>) {
        if delivered.is_empty() {
            return;
        }
        let serialized = {
            let mut memory = self.inner.sender_memory.entry(group.clone()).or_default();
            memory.extend(delivered);
            memory.iter().cloned().collect::<Vec<_>>().join("\n")
        };
        if let Err(err) =
            self.inner.store.set(KeyKind::SenderKeyMemory, &group.to_string(), serialized.into_bytes()).await
        {
            tracing::warn!(%err, "failed to persist sender key memory");
        }
    }

    pub(crate) async fn handle_retry_receipt(&self, node: &Node, chat: &Jid, requester: &Jid) -> Result<()> {
        let Some(id) = node.attr_string("id") else { return Ok(()) };
        let cached = self.sent_message(&id);
        let stored = if cached.is_none() { self.stored_send(&id).await } else { None };
        let Some(sent) = cached.or(stored) else {
            tracing::debug!(%id, "retry requested for a message we no longer keep");
            return Ok(());
        };
        if self.bump_retry(&format!("resend:{id}:{requester}")) > MAX_RESENDS {
            return Ok(());
        }
        let retry = node.child("retry");
        let count = retry.and_then(|r| r.attr_u64("count")).unwrap_or(1);
        let address = requester.signal_address();
        let signal = self.signal();

        let registration = node.child_uint("registration");
        match node.child("keys").and_then(|keys| parse_bundle(keys, registration)) {
            Some(bundle) => signal.inject_bundle(&address, &bundle).await?,
            None if count >= 2 || !signal.missing_sessions(std::slice::from_ref(&address)).await?.is_empty() => {
                self.fetch_sessions(std::slice::from_ref(requester), true).await?
            }
            None => {}
        }

        let mut message = sent.message.clone();
        if chat.is_group() || chat.is_status_broadcast() {
            if let Some(own) = &sent.sender_address {
                if let Some(distribution) = signal.group_distribution(&chat.to_string(), own).await? {
                    message.sender_key_distribution_message = Some(Box::new(SenderKeyDistributionMessage {
                        group_id: Some(chat.to_string()),
                        axolotl_sender_key_distribution_message: Some(distribution),
                    }));
                }
            }
        } else if self.is_me(requester) {
            message = Message {
                device_sent_message: Some(Box::new(DeviceSentMessage {
                    destination_jid: Some(sent.chat.to_string()),
                    message: Some(Box::new(message)),
                    phash: None,
                })),
                ..Default::default()
            };
        }

        let attrs = EncAttrs::of(&sent.message);
        let (kind, ciphertext) = signal.encrypt(&address, &pad(message.encode_to_vec())).await?;
        let mut stanza = Node::new("message").attr("id", id).attr("type", sent.message.stanza_type());
        for key in ["from", "participant", "recipient"] {
            if let Some(value) = node.get(key) {
                stanza.set_attr(if key == "from" { "to" } else { key }, value.clone());
            }
        }
        if !chat.is_group() && !chat.is_status_broadcast() {
            stanza.set_attr("device_fanout", "false");
        }
        let mut content = vec![enc_node(kind.as_str(), ciphertext, attrs).attr("count", count)];
        if kind == EncType::PreKey {
            if let Some(account) = self.creds().account {
                content.push(Node::new("device-identity").bytes(account));
            }
        }
        self.send_node(&stanza.children(content))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::store::MemoryStore;

    #[tokio::test]
    async fn small_futures() {
        let (client, _events) = Client::builder().store(Arc::new(MemoryStore::new())).build().await.unwrap();
        let chat = Jid::phone("5215511111111");
        let sizes = [
            std::mem::size_of_val(&client.send_text(&chat, "hola")),
            std::mem::size_of_val(&client.send_message(&chat, Message::text("hola"))),
            std::mem::size_of_val(&client.send_image(&chat, &[], None)),
        ];
        assert!(sizes.iter().all(|&size| size < 4096), "send futures must stay small: {sizes:?}");
    }
}
