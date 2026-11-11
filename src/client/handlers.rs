use std::sync::atomic::Ordering;

use super::{Client, Outcome};
use crate::binary::Node;
use crate::error::Result;
use crate::events::{
    BlocklistChange, CallEvent, CallKind, ChatState, ChatStateKind, Event, GroupAction, GroupUpdate, JoinVia,
    LogoutCause, Presence, Receipt, ReceiptType, ServerAck,
};
use crate::jid::Jid;
use crate::message::now_secs;
use crate::store::KeyKind;

const LOGGED_OUT_CODES: [&str; 2] = ["401", "419"];

fn one_line(node: &Node) -> String {
    let attrs: String = node.attrs.iter().map(|(key, value)| format!(" {key}=\"{value}\"")).collect();
    let children: Vec<String> = node.nodes().iter().map(one_line).collect();
    if children.is_empty() {
        format!("<{}{attrs}/>", node.tag)
    } else {
        format!("<{}{attrs}>{}</{}>", node.tag, children.join(""), node.tag)
    }
}

impl Client {
    pub(super) async fn handle_node(&self, node: Node) {
        let result = match node.tag.as_ref() {
            "iq" => self.on_iq(&node).await,
            "ib" => self.on_ib(&node).await,
            "receipt" => self.on_receipt(&node).await,
            "notification" => self.on_notification(node).await,
            "call" => self.on_call(&node),
            "presence" => self.on_presence(&node),
            "chatstate" => self.on_chat_state(&node),
            "ack" => {
                self.on_ack(&node);
                Ok(())
            }
            // Reactions to your own status updates come as <status type="reaction">. Left without an ack, the
            // server ends the stream 50 minutes later and sends it again on the next connection, forever.
            // Found by the soak on a real number: 17 forced reconnects in 19 h.
            "status" => {
                self.send_ack(&node, None);
                self.emit(Event::Unhandled(node));
                Ok(())
            }
            _ => {
                self.emit(Event::Unhandled(node));
                Ok(())
            }
        };
        if let Err(err) = result {
            tracing::warn!(%err, "failed to handle stanza");
        }
    }

    pub(super) fn end_on_terminal(&self, node: &Node) -> bool {
        let outcome = match node.tag.as_ref() {
            "failure" => self.failure_outcome(node),
            "stream:error" => self.stream_error_outcome(node),
            "xmlstreamend" => Outcome::Lost("the server ended the stream".into()),
            _ => return false,
        };
        self.end_link(outcome);
        true
    }

    // WhatsApp wants an ack for every message, receipt, notification and call. What hasn't been acked yet is
    // kept (tag, type and child tags only) so a stream error that points at an ack can say which one it was.
    pub(super) fn expect_ack(&self, node: &Node) {
        if matches!(node.tag.as_ref(), "iq" | "ib" | "ack" | "presence" | "chatstate" | "success" | "failure") {
            return;
        }
        let Some(id) = node.attr_str("id") else { return };
        let mut summary = format!("<{}", node.tag);
        if let Some(kind) = node.attr_str("type") {
            summary.push_str(&format!(" type=\"{kind}\""));
        }
        let children: Vec<&str> = node.nodes().iter().map(|child| child.tag.as_ref()).collect();
        summary.push_str(&format!("> [{}]", children.join(",")));
        let mut unacked = self.inner.unacked.lock();
        if unacked.len() >= 4096 {
            unacked.retain(|_, (_, at)| at.elapsed() < std::time::Duration::from_secs(600));
        }
        unacked.insert(format!("{}:{id}", node.tag), (summary, std::time::Instant::now()));
    }

    #[cfg(test)]
    pub(crate) fn unacked_count(&self) -> usize {
        self.inner.unacked.lock().len()
    }

    fn still_unacked(&self) -> Vec<String> {
        let unacked = self.inner.unacked.lock();
        let mut old: Vec<_> = unacked.values().filter(|(_, at)| at.elapsed().as_secs() >= 30).collect();
        old.sort_by_key(|(_, at)| *at);
        old.iter().take(3).map(|(summary, at)| format!("{summary} {}s", at.elapsed().as_secs())).collect()
    }

    pub(crate) fn send_ack(&self, node: &Node, error: Option<u16>) {
        if let Some(id) = node.attr_str("id") {
            self.inner.unacked.lock().remove(&format!("{}:{id}", node.tag));
        }
        let mut ack = Node::new("ack").attr("class", node.tag.clone());
        if let Some(id) = node.get("id") {
            ack.set_attr("id", id.clone());
        }
        if let Some(from) = node.get("from") {
            ack.set_attr("to", from.clone());
        }
        for key in ["participant", "recipient"] {
            if let Some(value) = node.get(key) {
                ack.set_attr(key, value.clone());
            }
        }
        let unavailable = node.child("unavailable").is_some();
        if let Some(kind) = node.get("type") {
            if node.tag != "message" || unavailable || error.is_some() {
                ack.set_attr("type", kind.clone());
            }
        }
        if node.tag == "message" && unavailable {
            if let Some(me) = self.me() {
                ack.set_attr("from", me);
            }
        }
        if let Some(code) = error {
            ack.set_attr("error", code);
        }
        if let Err(err) = self.send_node(&ack) {
            tracing::debug!(%err, "could not send ack");
        }
    }

    pub(crate) async fn on_success(&self, node: &Node, generation: u64) -> Result<()> {
        if let Some(lid) = node.attr_jid("lid") {
            if self.lid().as_ref() != Some(&lid) {
                self.update_creds(|creds| creds.lid = Some(lid)).await?;
            }
        }
        if let Err(err) = self.ensure_pre_keys().await {
            tracing::warn!(%err, "pre-key maintenance failed");
        }
        let passive = Node::new("iq")
            .attr("to", Jid::server())
            .attr("xmlns", "passive")
            .attr("type", "set")
            .child_node(Node::new("active"));
        if let Err(err) = self.query(passive).await {
            tracing::warn!(%err, "failed to switch the session to active");
        }
        if self.current_generation() != generation {
            tracing::debug!("the connection that logged in is already gone");
            return Ok(());
        }
        self.inner.connected.store(true, Ordering::Release);
        if self.inner.config.online_on_connect {
            if let Err(err) = self.set_presence(true).await {
                tracing::debug!(%err, "could not mark the session as online");
            }
        }
        let client = self.clone();
        tokio::spawn(async move { client.warm_up().await });
        let client = self.clone();
        tokio::spawn(async move { client.flush_outbox(generation).await });
        self.emit(Event::Connected);
        Ok(())
    }

    fn on_ack(&self, node: &Node) {
        let confirms_a_send = node.attr_is("class", "message")
            || (node.attr_is("class", "receipt")
                && node.attr_str("id").is_some_and(|id| self.inner.in_flight.contains(id)));
        if !confirms_a_send {
            return;
        }
        let Some(id) = node.attr_string("id") else { return };
        let error = node.attr_u64("error").map(|code| code as u16);
        if let Some(code) = error {
            tracing::warn!(%id, code, "whatsapp rejected an outgoing message");
        }
        let (latency, timings) = {
            let sent = self.inner.sent.lock();
            let latency = sent.sent_at(&id).map(|at| at.elapsed());
            (latency, sent.timings(&id).map(|timings| super::SendTimings { server: latency, ..timings }))
        };
        if let (Some(timings), None) = (timings, error) {
            self.record_timings(timings);
        }
        self.inner.in_flight.remove(&id);
        if self.inner.config.outbox {
            let (client, id) = (self.clone(), id.clone());
            tokio::spawn(async move { client.unpark(&id).await });
        }
        let chat = node.attr_jid("from").unwrap_or_default();
        self.emit(Event::ServerAck(ServerAck { id, chat, error, latency, timings }));
    }

    async fn warm_up(&self) {
        let started = std::time::Instant::now();
        let all = super::appstate::COLLECTIONS.iter().map(|c| (*c).to_owned()).collect();
        self.sync_app_state_later(all, false);
        if let Err(err) = self.fetch_server_props().await {
            tracing::debug!(%err, "could not fetch server props");
        }
        if let Err(err) = self.maintain_signed_pre_key().await {
            tracing::warn!(%err, "signed pre-key rotation failed");
        }
        let own = [self.me(), self.lid()].into_iter().flatten().map(|j| j.to_non_device()).collect::<Vec<_>>();
        let own_devices = async {
            let devices = self.devices_for(&own).await?;
            self.ensure_sessions(&devices).await
        };
        let (devices, groups) = tokio::join!(own_devices, self.joined_groups());
        if let Err(err) = devices {
            tracing::debug!(%err, "could not warm up our own devices");
        }
        let groups = match groups {
            Ok(groups) => groups,
            Err(err) => {
                tracing::debug!(%err, "could not warm up group metadata");
                return;
            }
        };
        let members: Vec<Jid> = groups.iter().flat_map(|g| g.participants.iter().map(|p| p.jid.clone())).collect();
        let fetched = self.prefetch_devices(members).await;
        tracing::debug!(groups = groups.len(), fetched, elapsed = ?started.elapsed(), "caches warmed up");
    }

    fn failure_outcome(&self, node: &Node) -> Outcome {
        let reason = node.attr_str("reason").unwrap_or("500");
        let blocked = |code: u16, why: &str| Outcome::Blocked {
            code,
            reason: why.to_owned(),
            expires: node.attr_u64("expire").or_else(|| node.attr_u64("expires")),
        };
        let outcome = match reason {
            "402" => blocked(402, "the account is temporarily banned"),
            "403" => blocked(403, "the account is locked, open WhatsApp on the phone to unlock it"),
            "406" => blocked(406, "the account is banned"),
            code if LOGGED_OUT_CODES.contains(&code) => Outcome::LoggedOut {
                reason: format!("whatsapp refused the saved session at login (failure {code})"),
                cause: LogoutCause::SessionInvalidated,
                code: code.parse().ok(),
            },
            "405" => Outcome::Outdated,
            code => Outcome::Lost(format!("login failure {code}")),
        };
        outcome
    }

    fn stream_error_outcome(&self, node: &Node) -> Outcome {
        let code = node.attr_str("code").unwrap_or_default();
        let reason = node.first_child();
        let outcome = match (code, reason.map(|r| r.tag.as_ref())) {
            ("515", _) => Outcome::Restart,
            (_, Some("conflict")) if reason.is_some_and(|r| r.attr_is("type", "device_removed")) => {
                Outcome::LoggedOut {
                    reason: "the phone removed this linked device".into(),
                    cause: LogoutCause::RemovedFromPhone,
                    code: code.parse().ok(),
                }
            }
            ("401", tag) => Outcome::LoggedOut {
                reason: format!("whatsapp ended the session (stream error 401 {})", tag.unwrap_or("without detail")),
                cause: LogoutCause::SessionInvalidated,
                code: Some(401),
            },
            (_, Some("conflict")) => Outcome::Replaced,
            (code, _) => {
                let detail = reason.map_or_else(|| "without detail".to_owned(), one_line);
                let unacked = self.still_unacked();
                let unacked =
                    if unacked.is_empty() { String::new() } else { format!("; unacked: {}", unacked.join(", ")) };
                Outcome::Lost(format!("stream error {code} {detail}{unacked}"))
            }
        };
        outcome
    }

    async fn on_iq(&self, node: &Node) -> Result<()> {
        if node.child("pair-device").is_some() {
            return self.on_pair_device(node).await;
        }
        if node.child("pair-success").is_some() {
            return self.on_pair_success(node).await;
        }
        if node.attr_is("xmlns", "urn:xmpp:ping") {
            let mut pong = Node::new("iq").attr("to", Jid::server()).attr("type", "result");
            if let Some(id) = node.get("id") {
                pong.set_attr("id", id.clone());
            }
            return self.send_node(&pong);
        }
        self.emit(Event::Unhandled(node.clone()));
        Ok(())
    }

    async fn on_ib(&self, node: &Node) -> Result<()> {
        for child in node.nodes() {
            match child.tag.as_ref() {
                "offline" => self.emit(Event::OfflineSyncComplete { count: child.attr_u64("count").unwrap_or(0) }),
                "offline_preview" => {
                    let batch = Node::new("ib").child_node(Node::new("offline_batch").attr("count", "100"));
                    self.send_node(&batch)?;
                }
                "edge_routing" => {
                    if let Some(routing) = child.child_bytes("routing_info").map(<[u8]>::to_vec) {
                        self.update_creds(|creds| creds.routing_info = Some(routing)).await?;
                    }
                }
                "dirty" => {
                    let mut clean = Node::new("clean");
                    if let Some(kind) = child.get("type") {
                        clean.set_attr("type", kind.clone());
                    }
                    if let Some(timestamp) = child.get("timestamp") {
                        clean.set_attr("timestamp", timestamp.clone());
                    }
                    let request = Node::new("iq")
                        .attr("to", Jid::server())
                        .attr("type", "set")
                        .attr("xmlns", "urn:xmpp:whatsapp:dirty")
                        .child_node(clean);
                    self.query(request).await?;
                }
                "downgrade_webclient" => self.end_link(Outcome::Lost("multi-device is not enabled".into())),
                _ => {}
            }
        }
        Ok(())
    }

    async fn on_receipt(&self, node: &Node) -> Result<()> {
        let Some(from) = node.attr_jid("from") else {
            self.send_ack(node, None);
            return Ok(());
        };
        let participant = node.attr_jid("participant");
        let kind = ReceiptType::parse(node.attr_str("type"));
        let mut ids: Vec<String> = node.attr_string("id").into_iter().collect();
        if let Some(list) = node.child("list") {
            ids.extend(list.children_by("item").filter_map(|item| item.attr_string("id")));
        }
        let sender = participant.clone().unwrap_or_else(|| from.clone());
        let chat = match node.attr_jid("recipient") {
            Some(recipient) if !from.is_group() => recipient,
            _ => from.to_non_device(),
        };
        let from_me = self.is_me(&sender);

        if kind == ReceiptType::Retry {
            if let Err(err) = self.handle_retry_receipt(node, &chat, &sender).await {
                tracing::warn!(%err, "could not resend message after retry receipt");
            }
        }
        self.emit(Event::Receipt(Receipt {
            chat,
            sender,
            ids,
            kind,
            timestamp: node.attr_i64("t").unwrap_or_else(now_secs),
            from_me,
        }));
        self.send_ack(node, None);
        Ok(())
    }

    async fn on_notification(&self, node: Node) -> Result<()> {
        let kind = node.attr_str("type").unwrap_or_default().to_owned();
        let result = match kind.as_str() {
            "encrypt" => self.on_encrypt_notification(&node).await,
            "w:gp2" => self.on_group_notification(&node).await,
            "devices" => {
                if let Some(from) = node.attr_jid("from") {
                    self.devices_changed(&from).await;
                }
                Ok(())
            }
            "link_code_companion_reg" => self.on_link_code_notification(&node).await,
            "privacy_token" => self.on_privacy_token_notification(&node).await,
            "mediaretry" => {
                if let Some((_, waiter)) = node.attr_str("id").and_then(|id| self.inner.media_retries.remove(id)) {
                    let _ = waiter.send(node.clone());
                }
                Ok(())
            }
            "server_sync" => {
                let collections: Vec<String> =
                    node.children_by("collection").filter_map(|c| c.attr_string("name")).collect();
                if !collections.is_empty() {
                    self.sync_app_state_later(collections, false);
                }
                Ok(())
            }
            "picture" => {
                self.on_picture_notification(&node);
                self.emit(Event::Notification(node.clone()));
                Ok(())
            }
            "status" => {
                self.on_status_notification(&node);
                self.emit(Event::Notification(node.clone()));
                Ok(())
            }
            "account_sync" => {
                self.on_account_sync_notification(&node);
                self.emit(Event::Notification(node.clone()));
                Ok(())
            }
            _ => {
                self.emit(Event::Notification(node.clone()));
                Ok(())
            }
        };
        self.send_ack(&node, None);
        result
    }

    fn on_picture_notification(&self, node: &Node) {
        let timestamp = node.attr_i64("t").unwrap_or_else(now_secs);
        for child in node.nodes() {
            let removed = match child.tag.as_ref() {
                "delete" => true,
                "set" | "add" => false,
                _ => continue,
            };
            let Some(jid) = child.attr_jid("jid") else { continue };
            self.emit(Event::PictureChanged {
                jid,
                author: child.attr_jid("author"),
                picture_id: child.attr_string("id"),
                removed,
                timestamp,
            });
        }
    }

    fn on_status_notification(&self, node: &Node) {
        let (Some(jid), Some(about)) = (node.attr_jid("from"), node.child("set").and_then(Node::content_string)) else {
            return;
        };
        self.emit(Event::AboutChanged { jid, about, timestamp: node.attr_i64("t").unwrap_or_else(now_secs) });
    }

    fn on_account_sync_notification(&self, node: &Node) {
        if let Some(blocklist) = node.child("blocklist") {
            let items: Vec<BlocklistChange> = blocklist
                .nodes()
                .iter()
                .filter_map(|item| {
                    Some(BlocklistChange {
                        jid: item.attr_jid("jid")?,
                        phone: item.attr_jid("pn_jid"),
                        blocked: !(item.attr_is("action", "unblock") || item.attr_is("active", "false")),
                    })
                })
                .collect();
            if !items.is_empty() {
                self.emit(Event::BlocklistChanged { items });
            }
        }
        if let Some(privacy) = node.child("privacy") {
            let settings: Vec<(String, String)> = privacy
                .nodes()
                .iter()
                .filter_map(|category| Some((category.attr_string("name")?, category.attr_string("value")?)))
                .collect();
            if !settings.is_empty() {
                self.emit(Event::PrivacyChanged { settings });
            }
        }
        if node.child("picture").is_some() {
            if let Some(me) = self.me() {
                self.emit(Event::PictureChanged {
                    jid: me.to_non_device(),
                    author: None,
                    picture_id: None,
                    removed: false,
                    timestamp: node.attr_i64("t").unwrap_or_else(now_secs),
                });
            }
        }
    }

    async fn on_encrypt_notification(&self, node: &Node) -> Result<()> {
        if let Some(count) = node.child("count") {
            if count.attr_u64("value").unwrap_or(0) < u64::from(super::prekeys::MIN_PRE_KEYS) {
                self.upload_pre_keys(super::prekeys::UPLOAD_BATCH).await?;
            }
        }
        if node.child("identity").is_some() {
            if let Some(from) = node.attr_jid("from") {
                self.devices_changed(&from).await;
            }
        }
        Ok(())
    }

    async fn on_group_notification(&self, node: &Node) -> Result<()> {
        let Some(group) = node.attr_jid("from") else { return Ok(()) };
        self.forget_group(&group).await;
        self.refresh_group_later(group.clone());
        let author = node.attr_jid("participant");
        let timestamp = node.attr_i64("t").unwrap_or_else(now_secs);
        for child in node.nodes() {
            // add/remove/promote list participants right under the action, but `create` nests them
            // inside <group>. Missing that is why "the bot joined" never fired for brand new groups.
            let listed = if child.tag.as_ref() == "create" { child.child("group").unwrap_or(child) } else { child };
            let participants: Vec<Jid> = listed.children_by("participant").filter_map(|p| p.attr_jid("jid")).collect();
            let action = match child.tag.as_ref() {
                "create" => GroupAction::Created,
                "add" => GroupAction::Add,
                "remove" => GroupAction::Remove,
                "promote" => GroupAction::Promote,
                "demote" => GroupAction::Demote,
                "leave" => GroupAction::Leave,
                "subject" => GroupAction::Subject(child.attr_string("subject").unwrap_or_default()),
                "description" => GroupAction::Description(child.child("body").and_then(Node::content_string)),
                "announcement" => GroupAction::Announce(true),
                "not_announcement" => GroupAction::Announce(false),
                "locked" => GroupAction::Locked(true),
                "unlocked" => GroupAction::Locked(false),
                "ephemeral" => GroupAction::Ephemeral(child.attr_u64("expiration").unwrap_or(0) as u32),
                "not_ephemeral" => GroupAction::Ephemeral(0),
                other => GroupAction::Other(other.to_owned()),
            };
            if matches!(action, GroupAction::Remove | GroupAction::Leave) {
                self.rotate_sender_key(&group).await?;
            }
            let includes_me = participants.iter().any(|p| self.is_me(p));
            let by_me = author.as_ref().is_some_and(|a| self.is_me(a));
            let joined = match &action {
                GroupAction::Created if includes_me || by_me => Some(JoinVia::Created),
                GroupAction::Add if includes_me && (by_me || child.attr_is("reason", "invite")) => {
                    Some(JoinVia::Invite)
                }
                GroupAction::Add if includes_me => Some(JoinVia::Added),
                _ => None,
            };
            let left = matches!(action, GroupAction::Remove | GroupAction::Leave) && includes_me;
            self.emit(Event::GroupUpdate(GroupUpdate {
                group: group.clone(),
                author: author.clone(),
                action,
                participants,
                timestamp,
            }));
            if let Some(via) = joined {
                self.announce_join(&group, author.clone().filter(|a| !self.is_me(a)), via);
            }
            if left {
                self.forget_join(&group);
                self.emit(Event::LeftGroup { group: group.clone(), author: author.clone(), removed: !by_me });
            }
        }
        Ok(())
    }

    pub(crate) async fn rotate_sender_key(&self, group: &Jid) -> Result<()> {
        let signal = self.signal();
        let group_name = group.to_string();
        let device = self.me().map_or(0, |me| me.device);
        for own in [self.me(), self.lid()].into_iter().flatten() {
            signal.reset_own_sender_key(&group_name, &own.with_device(device).signal_address()).await?;
        }
        self.inner.sender_memory.remove(group);
        self.inner.store.remove(KeyKind::SenderKeyMemory, &group_name).await
    }

    fn on_call(&self, node: &Node) -> Result<()> {
        self.send_ack(node, None);
        let Some(child) = node.first_child() else { return Ok(()) };
        let kind = match child.tag.as_ref() {
            "offer" | "offer_notice" => CallKind::Offer,
            "accept" => CallKind::Accept,
            "reject" => CallKind::Reject,
            "terminate" => CallKind::Terminate,
            _ => return Ok(()),
        };
        let from = child
            .attr_jid("call-creator")
            .or_else(|| child.attr_jid("from"))
            .or_else(|| node.attr_jid("from"))
            .unwrap_or_default();
        self.emit(Event::Call(CallEvent {
            id: child.attr_string("call-id").unwrap_or_default(),
            from,
            kind,
            video: child.child("video").is_some(),
            is_group: child.has("group-jid"),
            timestamp: node.attr_i64("t").unwrap_or_else(now_secs),
        }));
        Ok(())
    }

    fn on_presence(&self, node: &Node) -> Result<()> {
        let Some(from) = node.attr_jid("from") else { return Ok(()) };
        self.emit(Event::Presence(Presence {
            from,
            available: !matches!(node.attr_str("type"), Some("unavailable")),
            last_seen: node.attr_i64("last"),
        }));
        Ok(())
    }

    fn on_chat_state(&self, node: &Node) -> Result<()> {
        let Some(chat) = node.attr_jid("from") else { return Ok(()) };
        let sender = node.attr_jid("participant").unwrap_or_else(|| chat.clone());
        let Some(child) = node.first_child() else { return Ok(()) };
        let state = match (child.tag.as_ref(), child.attr_str("media")) {
            ("composing", Some("audio")) => ChatStateKind::Recording,
            ("composing", _) => ChatStateKind::Composing,
            _ => ChatStateKind::Paused,
        };
        self.emit(Event::ChatState(ChatState { chat, sender, state }));
        Ok(())
    }

    pub(crate) fn is_me(&self, jid: &Jid) -> bool {
        let creds = self.inner.creds.read();
        creds.me.as_ref().is_some_and(|me| me.same_user(jid))
            || creds.lid.as_ref().is_some_and(|lid| lid.same_user(jid))
    }
}
