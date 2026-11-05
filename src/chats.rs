use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::events::{ChatUpdate, Event, GroupAction, MessageEvent, MessageInfo, ReceiptType};
use crate::jid::Jid;
use crate::proto::{HistorySync, Message, ProtocolMessage, WebMessageInfo};

const DEFAULT_LIMIT: usize = 500;
const AUTOSAVE_EVERY: Duration = Duration::from_secs(10);

mod proto_b64 {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, M: prost::Message>(value: &M, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(value.encode_to_vec()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, M: prost::Message + Default>(deserializer: D) -> Result<M, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let bytes = STANDARD.decode(raw).map_err(D::Error::custom)?;
        M::decode(bytes.as_slice()).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageStatus {
    Error,
    #[default]
    Pending,
    Sent,
    Delivered,
    Read,
    Played,
}

impl MessageStatus {
    fn from_web(code: Option<i32>) -> Self {
        match code {
            Some(0) => MessageStatus::Error,
            Some(2) => MessageStatus::Sent,
            Some(3) => MessageStatus::Delivered,
            Some(4) => MessageStatus::Read,
            Some(5) => MessageStatus::Played,
            _ => MessageStatus::Pending,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Chat {
    pub jid: Jid,
    pub name: Option<String>,
    pub unread: u32,
    pub last_message_at: i64,
    pub archived: bool,
    pub pinned: bool,
    pub muted_until: Option<i64>,
    pub locked: bool,
    pub labels: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Contact {
    pub jid: Jid,
    pub name: Option<String>,
    pub first_name: Option<String>,
    pub push_name: Option<String>,
    pub lid: Option<Jid>,
    pub phone: Option<Jid>,
}

impl Contact {
    pub fn display_name(&self) -> Option<&str> {
        self.name.as_deref().or(self.push_name.as_deref()).or(self.first_name.as_deref())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub chat: Jid,
    pub sender: Option<Jid>,
    pub from_me: bool,
    pub timestamp: i64,
    pub push_name: Option<String>,
    #[serde(with = "proto_b64")]
    pub message: Message,
    #[serde(default)]
    pub status: MessageStatus,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub edited: bool,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub reactions: BTreeMap<String, String>,
    #[serde(default)]
    pub poll_votes: BTreeMap<String, Vec<String>>,
}

impl StoredMessage {
    pub fn text(&self) -> Option<&str> {
        self.message.get_text()
    }

    pub fn to_event(&self) -> MessageEvent {
        let sender = self.sender.clone().unwrap_or_else(|| self.chat.clone());
        MessageEvent {
            info: MessageInfo {
                id: self.id.clone(),
                is_group: self.chat.is_group(),
                chat: self.chat.clone(),
                sender,
                sender_alt: None,
                sender_phone: None,
                sender_lid: None,
                from_me: self.from_me,
                push_name: self.push_name.clone(),
                timestamp: self.timestamp,
                category: None,
                offline: true,
                edit: None,
                server_id: None,
                received_at: Instant::now(),
            },
            message: self.message.clone(),
            raw: self.message.clone(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "chat": self.chat.to_string(),
            "sender": self.sender.as_ref().map(Jid::to_string),
            "fromMe": self.from_me,
            "timestamp": self.timestamp,
            "pushName": self.push_name,
            "text": self.text(),
            "kind": self.message.stanza_type(),
            "status": self.status,
            "starred": self.starred,
            "edited": self.edited,
            "revoked": self.revoked,
            "reactions": self.reactions,
            "pollVotes": self.poll_votes,
        })
    }
}

#[derive(Default)]
struct ChatLog {
    timeline: BTreeSet<(i64, String)>,
    items: HashMap<String, StoredMessage>,
}

impl ChatLog {
    fn insert(&mut self, mut message: StoredMessage, limit: usize) {
        if let Some(old) = self.items.remove(&message.id) {
            self.timeline.remove(&(old.timestamp, old.id.clone()));
            message.status = message.status.max(old.status);
            message.starred |= old.starred;
            message.edited |= old.edited;
            message.revoked |= old.revoked;
            if old.edited || old.revoked {
                message.message = old.message;
            }
            let mut reactions = old.reactions;
            reactions.append(&mut message.reactions);
            message.reactions = reactions;
            let mut votes = old.poll_votes;
            votes.append(&mut message.poll_votes);
            message.poll_votes = votes;
        }
        self.timeline.insert((message.timestamp, message.id.clone()));
        self.items.insert(message.id.clone(), message);
        while self.items.len() > limit {
            let Some(oldest) = self.timeline.pop_first() else { break };
            self.items.remove(&oldest.1);
        }
    }

    fn remove(&mut self, id: &str) {
        if let Some(old) = self.items.remove(id) {
            self.timeline.remove(&(old.timestamp, old.id));
        }
    }
}

#[derive(Default)]
struct State {
    chats: HashMap<Jid, Chat>,
    contacts: HashMap<Jid, Contact>,
    logs: HashMap<Jid, ChatLog>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Snapshot {
    version: u32,
    chats: Vec<Chat>,
    contacts: Vec<Contact>,
    messages: Vec<StoredMessage>,
}

pub struct ChatStore {
    state: RwLock<State>,
    limit: usize,
    path: Option<PathBuf>,
    dirty: AtomicBool,
}

fn chat_key(jid: &Jid) -> Jid {
    jid.to_non_device()
}

impl Default for ChatStore {
    fn default() -> Self {
        ChatStore::new()
    }
}

impl ChatStore {
    pub fn new() -> Self {
        ChatStore::with_limit(DEFAULT_LIMIT)
    }

    pub fn with_limit(messages_per_chat: usize) -> Self {
        ChatStore {
            state: RwLock::new(State::default()),
            limit: messages_per_chat.max(1),
            path: None,
            dirty: AtomicBool::new(false),
        }
    }

    pub async fn open(path: impl AsRef<Path>) -> Result<Arc<ChatStore>> {
        ChatStore::open_with_limit(path, DEFAULT_LIMIT).await
    }

    pub async fn open_with_limit(path: impl AsRef<Path>, messages_per_chat: usize) -> Result<Arc<ChatStore>> {
        let path = path.as_ref().to_path_buf();
        let mut store = ChatStore::with_limit(messages_per_chat);
        store.path = Some(path.clone());
        match tokio::fs::read(&path).await {
            Ok(raw) => {
                let snapshot: Snapshot = serde_json::from_slice(&raw)?;
                store.restore(snapshot);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        let store = Arc::new(store);
        tokio::spawn(autosave(Arc::downgrade(&store)));
        Ok(store)
    }

    fn restore(&self, snapshot: Snapshot) {
        let mut state = self.state.write();
        for chat in snapshot.chats {
            state.chats.insert(chat.jid.clone(), chat);
        }
        for contact in snapshot.contacts {
            state.contacts.insert(contact.jid.clone(), contact);
        }
        for message in snapshot.messages {
            state.logs.entry(message.chat.clone()).or_default().insert(message, self.limit);
        }
    }

    fn snapshot(&self) -> Snapshot {
        let state = self.state.read();
        Snapshot {
            version: 1,
            chats: state.chats.values().cloned().collect(),
            contacts: state.contacts.values().cloned().collect(),
            messages: state.logs.values().flat_map(|log| log.items.values().cloned()).collect(),
        }
    }

    pub async fn save(&self) -> Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        self.dirty.store(false, Ordering::Release);
        let bytes = serde_json::to_vec(&self.snapshot())?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = path.with_extension("tmp");
        tokio::fs::write(&temporary, bytes).await?;
        tokio::fs::rename(&temporary, path).await.map_err(Error::from)
    }

    pub async fn save_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = serde_json::to_vec(&self.snapshot())?;
        tokio::fs::write(path, bytes).await.map_err(Error::from)
    }

    fn touch(&self) {
        self.dirty.store(true, Ordering::Release);
    }

    pub fn chats(&self) -> Vec<Chat> {
        let mut chats: Vec<Chat> = self.state.read().chats.values().cloned().collect();
        chats.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.last_message_at.cmp(&a.last_message_at)));
        chats
    }

    pub fn chat(&self, jid: &Jid) -> Option<Chat> {
        self.state.read().chats.get(&chat_key(jid)).cloned()
    }

    pub fn unread_total(&self) -> u32 {
        self.state.read().chats.values().map(|c| c.unread).sum()
    }

    pub fn contacts(&self) -> Vec<Contact> {
        self.state.read().contacts.values().cloned().collect()
    }

    pub fn contact(&self, jid: &Jid) -> Option<Contact> {
        self.state.read().contacts.get(&chat_key(jid)).cloned()
    }

    pub fn name_of(&self, jid: &Jid) -> Option<String> {
        let key = chat_key(jid);
        let state = self.state.read();
        state
            .contacts
            .get(&key)
            .and_then(|c| c.display_name().map(str::to_owned))
            .or_else(|| state.chats.get(&key).and_then(|c| c.name.clone()))
    }

    pub fn messages(&self, chat: &Jid, limit: usize) -> Vec<StoredMessage> {
        self.messages_before(chat, i64::MAX, limit)
    }

    pub fn messages_before(&self, chat: &Jid, before: i64, limit: usize) -> Vec<StoredMessage> {
        let state = self.state.read();
        let Some(log) = state.logs.get(&chat_key(chat)) else { return Vec::new() };
        let mut page: Vec<StoredMessage> = log
            .timeline
            .iter()
            .rev()
            .filter(|(timestamp, _)| *timestamp < before)
            .take(limit)
            .filter_map(|(_, id)| log.items.get(id).cloned())
            .collect();
        page.reverse();
        page
    }

    pub fn message(&self, chat: &Jid, id: &str) -> Option<StoredMessage> {
        self.state.read().logs.get(&chat_key(chat)).and_then(|log| log.items.get(id).cloned())
    }

    pub fn find_message(&self, id: &str) -> Option<StoredMessage> {
        self.state.read().logs.values().find_map(|log| log.items.get(id).cloned())
    }

    pub fn search(&self, text: &str, limit: usize) -> Vec<StoredMessage> {
        let needle = text.to_lowercase();
        let state = self.state.read();
        let mut found: Vec<StoredMessage> = state
            .logs
            .values()
            .flat_map(|log| log.items.values())
            .filter(|m| !m.revoked && m.text().is_some_and(|t| t.to_lowercase().contains(&needle)))
            .cloned()
            .collect();
        found.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
        found.truncate(limit);
        found
    }

    fn with_message(&self, chat: &Jid, id: &str, apply: impl FnOnce(&mut StoredMessage)) {
        let mut state = self.state.write();
        if let Some(message) = state.logs.get_mut(&chat_key(chat)).and_then(|log| log.items.get_mut(id)) {
            apply(message);
            self.touch();
        }
    }

    fn chat_entry<'a>(state: &'a mut State, jid: &Jid) -> &'a mut Chat {
        let key = chat_key(jid);
        state.chats.entry(key.clone()).or_insert_with(|| Chat { jid: key, ..Default::default() })
    }

    fn contact_entry<'a>(state: &'a mut State, jid: &Jid) -> &'a mut Contact {
        let key = chat_key(jid);
        state.contacts.entry(key.clone()).or_insert_with(|| Contact { jid: key, ..Default::default() })
    }

    pub(crate) fn record_sent(&self, chat: &Jid, id: &str, sender: Option<Jid>, message: &Message) {
        let event = MessageEvent {
            info: MessageInfo {
                id: id.to_owned(),
                is_group: chat.is_group(),
                chat: chat.clone(),
                sender: sender.unwrap_or_else(|| chat.clone()),
                sender_alt: None,
                sender_phone: None,
                sender_lid: None,
                from_me: true,
                push_name: None,
                timestamp: crate::message::now_secs(),
                category: None,
                offline: false,
                edit: None,
                server_id: None,
                received_at: Instant::now(),
            },
            message: message.unwrapped().clone(),
            raw: message.clone(),
        };
        self.ingest_message(&event, MessageStatus::Pending);
    }

    fn insert(&self, message: StoredMessage, counts_as_unread: bool) {
        let mut state = self.state.write();
        let chat = Self::chat_entry(&mut state, &message.chat);
        chat.last_message_at = chat.last_message_at.max(message.timestamp);
        if counts_as_unread && !message.from_me {
            chat.unread += 1;
        }
        if message.from_me {
            chat.unread = 0;
        }
        let key = message.chat.clone();
        state.logs.entry(key).or_default().insert(message, self.limit);
        self.touch();
    }

    pub fn ingest(&self, event: &Event) {
        match event {
            Event::Message(message) => {
                let status = if message.info.from_me { MessageStatus::Sent } else { MessageStatus::Delivered };
                self.ingest_message(message, status)
            }
            Event::Receipt(receipt) => {
                let status = match receipt.kind {
                    ReceiptType::Delivered => Some(MessageStatus::Delivered),
                    ReceiptType::Read => Some(MessageStatus::Read),
                    ReceiptType::Played => Some(MessageStatus::Played),
                    _ => None,
                };
                if matches!(receipt.kind, ReceiptType::ReadSelf) {
                    let mut state = self.state.write();
                    if let Some(chat) = state.chats.get_mut(&chat_key(&receipt.chat)) {
                        chat.unread = 0;
                        self.touch();
                    }
                }
                if let Some(status) = status {
                    for id in &receipt.ids {
                        self.with_message(&receipt.chat, id, |m| m.status = m.status.max(status));
                    }
                }
            }
            Event::ServerAck(ack) => {
                let status = if ack.error.is_some() { MessageStatus::Error } else { MessageStatus::Sent };
                self.with_message(&ack.chat, &ack.id, |m| {
                    if m.status == MessageStatus::Pending || status == MessageStatus::Error {
                        m.status = status;
                    }
                });
            }
            Event::PollVote(vote) => {
                let voter = vote.voter.to_non_device().to_string();
                self.with_message(&vote.chat, &vote.poll_id, |m| {
                    if vote.selected.is_empty() {
                        m.poll_votes.remove(&voter);
                    } else {
                        m.poll_votes.insert(voter, vote.selected.clone());
                    }
                });
            }
            Event::HistorySync(history) => self.ingest_history(history),
            Event::ChatUpdate { update, .. } => self.ingest_chat_update(update),
            Event::GroupUpdate(update) => {
                if let GroupAction::Subject(subject) = &update.action {
                    let mut state = self.state.write();
                    Self::chat_entry(&mut state, &update.group).name = Some(subject.clone());
                    self.touch();
                }
            }
            _ => {}
        }
    }

    fn ingest_message(&self, event: &MessageEvent, status: MessageStatus) {
        let info = &event.info;
        let message = &event.message;
        if !info.from_me {
            if let Some(name) = info.push_name.as_ref().filter(|n| !n.is_empty()) {
                let mut state = self.state.write();
                Self::contact_entry(&mut state, &info.sender).push_name = Some(name.clone());
            }
        }
        if let Some(reaction) = &message.reaction_message {
            let Some(target) = reaction.key.as_ref().and_then(|k| k.id.as_deref()) else { return };
            let reactor = info.sender.to_non_device().to_string();
            self.with_message(&info.chat, target, |m| match reaction.text.as_deref() {
                Some(emoji) if !emoji.is_empty() => drop(m.reactions.insert(reactor, emoji.to_owned())),
                _ => drop(m.reactions.remove(&reactor)),
            });
            return;
        }
        if let Some(protocol) = &message.protocol_message {
            let Some(target) = protocol.key.as_ref().and_then(|k| k.id.as_deref()) else { return };
            match protocol.r#type {
                Some(ProtocolMessage::REVOKE) => self.with_message(&info.chat, target, |m| {
                    m.revoked = true;
                    m.message = Message::default();
                }),
                Some(ProtocolMessage::MESSAGE_EDIT) => {
                    if let Some(edited) = protocol.edited_message.as_deref() {
                        self.with_message(&info.chat, target, |m| {
                            m.message = edited.clone();
                            m.edited = true;
                        });
                    }
                }
                _ => {}
            }
            return;
        }
        if !worth_storing(message) {
            return;
        }
        let stored = StoredMessage {
            id: info.id.clone(),
            chat: chat_key(&info.chat),
            sender: Some(info.sender.to_non_device()),
            from_me: info.from_me,
            timestamp: info.timestamp,
            push_name: info.push_name.clone(),
            message: message.clone(),
            status,
            starred: false,
            edited: false,
            revoked: false,
            reactions: BTreeMap::new(),
            poll_votes: BTreeMap::new(),
        };
        self.insert(stored, true);
    }

    fn ingest_history(&self, history: &HistorySync) {
        {
            let mut state = self.state.write();
            for pushname in &history.pushnames {
                let (Some(id), Some(name)) = (pushname.id.as_deref(), pushname.pushname.as_ref()) else { continue };
                if let Ok(jid) = id.parse::<Jid>() {
                    Self::contact_entry(&mut state, &jid).push_name = Some(name.clone());
                }
            }
            for mapping in &history.phone_number_to_lid_mappings {
                let (Some(pn), Some(lid)) = (mapping.pn_jid.as_deref(), mapping.lid_jid.as_deref()) else { continue };
                let (Ok(pn), Ok(lid)) = (pn.parse::<Jid>(), lid.parse::<Jid>()) else { continue };
                Self::contact_entry(&mut state, &pn).lid = Some(lid.clone());
                Self::contact_entry(&mut state, &lid).phone = Some(pn);
            }
            for conversation in &history.conversations {
                let Ok(jid) = conversation.id.parse::<Jid>() else { continue };
                let chat = Self::chat_entry(&mut state, &jid);
                chat.name = conversation.name.clone().or(chat.name.take());
                chat.unread = conversation.unread_count.unwrap_or(chat.unread);
                chat.archived = conversation.archived.unwrap_or(chat.archived);
                chat.pinned = conversation.pinned.map_or(chat.pinned, |p| p > 0);
                let stamp = conversation.conversation_timestamp.or(conversation.last_msg_timestamp);
                chat.last_message_at = chat.last_message_at.max(stamp.unwrap_or_default() as i64);
            }
            self.touch();
        }
        for conversation in &history.conversations {
            for entry in &conversation.messages {
                if let Some(stored) = entry.message.as_ref().and_then(stored_from_web) {
                    self.insert(stored, false);
                }
            }
        }
    }

    fn ingest_chat_update(&self, update: &ChatUpdate) {
        let mut state = self.state.write();
        match update {
            ChatUpdate::Muted { chat, until } => Self::chat_entry(&mut state, chat).muted_until = *until,
            ChatUpdate::Archived { chat, archived } => Self::chat_entry(&mut state, chat).archived = *archived,
            ChatUpdate::Pinned { chat, pinned } => Self::chat_entry(&mut state, chat).pinned = *pinned,
            ChatUpdate::Locked { chat, locked } => Self::chat_entry(&mut state, chat).locked = *locked,
            ChatUpdate::MarkedRead { chat, read } => {
                let entry = Self::chat_entry(&mut state, chat);
                entry.unread = if *read { 0 } else { entry.unread.max(1) };
            }
            ChatUpdate::Deleted { chat } => {
                let key = chat_key(chat);
                state.chats.remove(&key);
                state.logs.remove(&key);
            }
            ChatUpdate::Cleared { chat } => {
                state.logs.remove(&chat_key(chat));
            }
            ChatUpdate::Starred { chat, message_id, starred, .. } => {
                if let Some(m) = state.logs.get_mut(&chat_key(chat)).and_then(|l| l.items.get_mut(message_id)) {
                    m.starred = *starred;
                }
            }
            ChatUpdate::DeletedForMe { chat, message_id, .. } => {
                if let Some(log) = state.logs.get_mut(&chat_key(chat)) {
                    log.remove(message_id);
                }
            }
            ChatUpdate::Contact { jid, full_name, first_name, lid, phone } => {
                let contact = Self::contact_entry(&mut state, jid);
                contact.name = full_name.clone().or(contact.name.take());
                contact.first_name = first_name.clone().or(contact.first_name.take());
                contact.lid = lid.clone().or(contact.lid.take());
                contact.phone = phone.clone().or(contact.phone.take());
            }
            ChatUpdate::ContactRemoved { jid } => {
                if let Some(contact) = state.contacts.get_mut(&chat_key(jid)) {
                    contact.name = None;
                    contact.first_name = None;
                }
            }
            ChatUpdate::ChatLabel { label, chat, labeled } => {
                let labels = &mut Self::chat_entry(&mut state, chat).labels;
                if *labeled {
                    labels.insert(label.clone());
                } else {
                    labels.remove(label);
                }
            }
            _ => return,
        }
        self.touch();
    }
}

fn worth_storing(message: &Message) -> bool {
    let m = message.unwrapped();
    m.has_content()
        && m.reaction_message.is_none()
        && m.protocol_message.is_none()
        && m.poll_update_message.is_none()
        && m.enc_event_response_message.is_none()
        && m.sender_key_distribution_message.is_none()
}

fn stored_from_web(info: &WebMessageInfo) -> Option<StoredMessage> {
    let key = info.key.as_ref()?;
    let chat: Jid = key.remote_jid.as_deref()?.parse().ok()?;
    let message = info.message.as_deref()?.unwrapped().clone();
    if !worth_storing(&message) {
        return None;
    }
    let from_me = key.from_me.unwrap_or(false);
    let sender = match key.participant.as_deref().or(info.participant.as_deref()) {
        Some(participant) => participant.parse::<Jid>().ok().map(|j| j.to_non_device()),
        None if from_me => None,
        None => Some(chat.to_non_device()),
    };
    Some(StoredMessage {
        id: key.id.clone()?,
        chat: chat_key(&chat),
        sender,
        from_me,
        timestamp: info.message_timestamp.unwrap_or_default() as i64,
        push_name: info.push_name.clone(),
        message,
        status: MessageStatus::from_web(info.status),
        starred: info.starred.unwrap_or(false),
        edited: false,
        revoked: false,
        reactions: BTreeMap::new(),
        poll_votes: BTreeMap::new(),
    })
}

async fn autosave(store: Weak<ChatStore>) {
    let mut tick = tokio::time::interval(AUTOSAVE_EVERY);
    tick.tick().await;
    loop {
        tick.tick().await;
        let Some(store) = store.upgrade() else { break };
        if store.dirty.load(Ordering::Acquire) {
            if let Err(err) = store.save().await {
                tracing::warn!(%err, "could not save the chat store");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{PollVote, Receipt, ServerAck};
    use crate::proto::{Conversation, HistorySyncMsg, MessageKey, Pushname, ReactionMessage};

    fn incoming(id: &str, chat: &Jid, sender: &Jid, text: &str, timestamp: i64) -> Event {
        let message = Message::text(text);
        Event::Message(Box::new(MessageEvent {
            info: MessageInfo {
                id: id.into(),
                chat: chat.clone(),
                sender: sender.clone(),
                sender_alt: None,
                sender_phone: None,
                sender_lid: None,
                from_me: false,
                is_group: chat.is_group(),
                push_name: Some("Ana".into()),
                timestamp,
                category: None,
                offline: false,
                edit: None,
                server_id: None,
                received_at: Instant::now(),
            },
            raw: message.clone(),
            message,
        }))
    }

    fn key(chat: &Jid, id: &str) -> MessageKey {
        MessageKey { remote_jid: Some(chat.to_string()), from_me: Some(false), id: Some(id.into()), participant: None }
    }

    #[test]
    fn follows_a_conversation() {
        let store = ChatStore::with_limit(3);
        let ana = Jid::phone("5215511111111");
        let now = crate::message::now_secs();
        store.ingest(&incoming("1", &ana, &ana, "hola", now - 20));
        store.ingest(&incoming("2", &ana, &ana, "¿cómo estás?", now - 10));
        assert_eq!(store.chat(&ana).unwrap().unread, 2);
        assert_eq!(store.name_of(&ana).as_deref(), Some("Ana"));

        store.record_sent(&ana, "3", None, &Message::text("bien, ¿y tú?"));
        assert_eq!(store.chat(&ana).unwrap().unread, 0);
        store.ingest(&Event::ServerAck(ServerAck {
            id: "3".into(),
            chat: ana.clone(),
            error: None,
            latency: None,
            timings: None,
        }));
        store.ingest(&Event::Receipt(Receipt {
            chat: ana.clone(),
            sender: ana.clone(),
            ids: vec!["3".into()],
            kind: ReceiptType::Read,
            timestamp: 12,
            from_me: false,
        }));
        assert_eq!(store.message(&ana, "3").unwrap().status, MessageStatus::Read);

        let reaction = Message {
            reaction_message: Some(Box::new(ReactionMessage {
                key: Some(key(&ana, "1")),
                text: Some("🔥".into()),
                ..Default::default()
            })),
            ..Default::default()
        };
        let Event::Message(mut react) = incoming("4", &ana, &ana, "", now) else { unreachable!() };
        react.message = reaction;
        store.ingest(&Event::Message(react));
        assert_eq!(store.message(&ana, "1").unwrap().reactions.values().next().map(String::as_str), Some("🔥"));

        let edit = Message::edit(key(&ana, "2"), Message::text("¿qué tal?"));
        let Event::Message(mut edited) = incoming("5", &ana, &ana, "", now) else { unreachable!() };
        edited.message = edit;
        store.ingest(&Event::Message(edited));
        let second = store.message(&ana, "2").unwrap();
        assert!(second.edited && second.text() == Some("¿qué tal?"));

        let Event::Message(mut revoke) = incoming("6", &ana, &ana, "", now) else { unreachable!() };
        revoke.message = Message::revoke(key(&ana, "2"));
        store.ingest(&Event::Message(revoke));
        assert!(store.message(&ana, "2").unwrap().revoked);

        store.ingest(&incoming("7", &ana, &ana, "último", now + 5));
        let page = store.messages(&ana, 10);
        assert_eq!(page.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["2", "3", "7"]);
        assert_eq!(store.search("ÚLTIMO", 5).len(), 1);
        assert_eq!(store.messages_before(&ana, now + 5, 1)[0].id, "3");
    }

    #[test]
    fn learns_from_history_and_app_state() {
        let store = ChatStore::new();
        let group = Jid::group("120363000000000001");
        let ana = Jid::phone("5215511111111");
        let history = HistorySync {
            conversations: vec![Conversation {
                id: group.to_string(),
                name: Some("Equipo".into()),
                unread_count: Some(4),
                conversation_timestamp: Some(100),
                messages: vec![HistorySyncMsg {
                    message: Some(WebMessageInfo {
                        key: Some(MessageKey { participant: Some(ana.to_string()), ..key(&group, "old") }),
                        message: Some(Box::new(Message::text("de ayer"))),
                        message_timestamp: Some(90),
                        status: Some(4),
                        ..Default::default()
                    }),
                    msg_order_id: None,
                }],
                ..Default::default()
            }],
            pushnames: vec![Pushname { id: Some(ana.to_string()), pushname: Some("Ana".into()) }],
            ..Default::default()
        };
        store.ingest(&Event::HistorySync(Box::new(history)));
        let chat = store.chat(&group).unwrap();
        assert_eq!((chat.name.as_deref(), chat.unread, chat.last_message_at), (Some("Equipo"), 4, 100));
        let old = store.message(&group, "old").unwrap();
        assert_eq!((old.sender.as_ref(), old.status), (Some(&ana), MessageStatus::Read));
        assert_eq!(store.contact(&ana).unwrap().push_name.as_deref(), Some("Ana"));

        store.ingest(&Event::ChatUpdate {
            update: ChatUpdate::Archived { chat: group.clone(), archived: true },
            timestamp: 0,
            initial_sync: false,
        });
        store.ingest(&Event::ChatUpdate {
            update: ChatUpdate::Contact {
                jid: ana.clone(),
                full_name: Some("Ana Soblend".into()),
                first_name: Some("Ana".into()),
                lid: None,
                phone: None,
            },
            timestamp: 0,
            initial_sync: false,
        });
        store.ingest(&Event::PollVote(PollVote {
            chat: group.clone(),
            poll_id: "old".into(),
            voter: ana.clone(),
            selected: vec!["Tacos".into()],
            timestamp: 0,
        }));
        assert!(store.chat(&group).unwrap().archived);
        assert_eq!(store.name_of(&ana).as_deref(), Some("Ana Soblend"));
        assert_eq!(store.message(&group, "old").unwrap().poll_votes.len(), 1);
    }

    #[tokio::test]
    async fn persists_atomically() {
        let path = std::env::temp_dir().join(format!("soblendwa-chats-{}.json", crate::crypto::random_u32()));
        let ana = Jid::phone("5215511111111");
        {
            let store = ChatStore::open(&path).await.unwrap();
            store.ingest(&incoming("1", &ana, &ana, "guárdame", 5));
            store.save().await.unwrap();
        }
        let reopened = ChatStore::open(&path).await.unwrap();
        assert_eq!(reopened.message(&ana, "1").unwrap().text(), Some("guárdame"));
        let event = reopened.message(&ana, "1").unwrap().to_event();
        assert_eq!((event.text(), &event.info.sender), (Some("guárdame"), &ana));
        let _ = tokio::fs::remove_file(&path).await;
    }
}
