use std::time::{Duration, Instant};

use crate::binary::Node;
use crate::jid::Jid;
use crate::proto::{self, MessageKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisconnectReason {
    ConnectionLost(String),
    RestartRequired,
    Replaced,
    QrTimeout,
    ClientOutdated,
    AccountBlocked(u16),
    Manual,
}

#[derive(Clone, Debug)]
pub enum Event {
    Qr(String),
    PairSuccess { id: Jid, lid: Option<Jid>, platform: Option<String>, business_name: Option<String> },
    Connected,
    Disconnected { reason: DisconnectReason, reconnecting: bool },
    LoggedOut { reason: String },
    AccountBlocked { code: u16, reason: String, expires: Option<u64> },
    OfflineSyncComplete { count: u64 },
    Message(Box<MessageEvent>),
    UndecryptableMessage { info: Box<MessageInfo>, error: String },
    MessageRecovered { id: String, chat: Jid, sender: Jid },
    PictureChanged { jid: Jid, author: Option<Jid>, picture_id: Option<String>, removed: bool, timestamp: i64 },
    AboutChanged { jid: Jid, about: String, timestamp: i64 },
    BlocklistChanged { items: Vec<BlocklistChange> },
    PrivacyChanged { settings: Vec<(String, String)> },
    Receipt(Receipt),
    Presence(Presence),
    ChatState(ChatState),
    GroupUpdate(GroupUpdate),
    Call(CallEvent),
    PollVote(PollVote),
    EventResponse(EventResponse),
    HistorySync(Box<proto::HistorySync>),
    ServerAck(ServerAck),
    ChatUpdate { update: ChatUpdate, timestamp: i64, initial_sync: bool },
    Notification(Node),
    Unhandled(Node),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlocklistChange {
    pub jid: Jid,
    pub phone: Option<Jid>,
    pub blocked: bool,
}

#[derive(Clone, Debug)]
pub struct MessageInfo {
    pub id: String,
    pub chat: Jid,
    pub sender: Jid,
    pub sender_alt: Option<Jid>,
    pub sender_phone: Option<Jid>,
    pub sender_lid: Option<Jid>,
    pub from_me: bool,
    pub is_group: bool,
    pub push_name: Option<String>,
    pub timestamp: i64,
    pub category: Option<String>,
    pub offline: bool,
    pub edit: Option<String>,
    pub server_id: Option<String>,
    pub received_at: Instant,
}

impl MessageInfo {
    pub fn elapsed(&self) -> Duration {
        self.received_at.elapsed()
    }

    pub fn is_older_than(&self, age: Duration) -> bool {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        now.saturating_sub(self.timestamp) > age.as_secs() as i64
    }

    pub fn is_stale(&self, age: Duration) -> bool {
        self.offline || self.is_older_than(age)
    }

    pub fn key(&self) -> MessageKey {
        MessageKey {
            remote_jid: Some(self.chat.to_string()),
            from_me: Some(self.from_me),
            id: Some(self.id.clone()),
            participant: (self.is_group || self.chat.is_broadcast()).then(|| self.sender.to_non_device().to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MessageEvent {
    pub info: MessageInfo,
    pub message: proto::Message,
    pub raw: proto::Message,
}

impl MessageEvent {
    pub fn text(&self) -> Option<&str> {
        self.message.get_text()
    }

    pub fn key(&self) -> MessageKey {
        self.info.key()
    }

    pub fn chat(&self) -> &Jid {
        &self.info.chat
    }

    pub fn sender(&self) -> &Jid {
        &self.info.sender
    }

    pub fn is_reaction(&self) -> bool {
        self.message.reaction_message.is_some()
    }

    pub fn quoted(&self) -> Option<&proto::Message> {
        self.message.context_info().and_then(|c| c.quoted_message.as_deref())
    }

    pub fn mentions(&self) -> Vec<Jid> {
        self.message
            .context_info()
            .map(|c| c.mentioned_jid.iter().filter_map(|j| j.parse().ok()).collect())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiptType {
    Delivered,
    Sender,
    Read,
    ReadSelf,
    Played,
    PlayedSelf,
    Retry,
    Inactive,
    PeerMessage,
    HistorySync,
    ServerError,
    Other(String),
}

impl ReceiptType {
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        match raw {
            None | Some("") => ReceiptType::Delivered,
            Some("sender") => ReceiptType::Sender,
            Some("read") => ReceiptType::Read,
            Some("read-self") => ReceiptType::ReadSelf,
            Some("played") => ReceiptType::Played,
            Some("played-self") => ReceiptType::PlayedSelf,
            Some("retry") => ReceiptType::Retry,
            Some("inactive") => ReceiptType::Inactive,
            Some("peer_msg") => ReceiptType::PeerMessage,
            Some("hist_sync") => ReceiptType::HistorySync,
            Some("server-error") => ReceiptType::ServerError,
            Some(other) => ReceiptType::Other(other.to_owned()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Receipt {
    pub chat: Jid,
    pub sender: Jid,
    pub ids: Vec<String>,
    pub kind: ReceiptType,
    pub timestamp: i64,
    pub from_me: bool,
}

#[derive(Clone, Debug)]
pub struct Presence {
    pub from: Jid,
    pub available: bool,
    pub last_seen: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatStateKind {
    Composing,
    Recording,
    Paused,
}

#[derive(Clone, Debug)]
pub struct ChatState {
    pub chat: Jid,
    pub sender: Jid,
    pub state: ChatStateKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupAction {
    Created,
    Add,
    Remove,
    Promote,
    Demote,
    Leave,
    Subject(String),
    Description(Option<String>),
    Announce(bool),
    Locked(bool),
    Ephemeral(u32),
    Other(String),
}

#[derive(Clone, Debug)]
pub struct GroupUpdate {
    pub group: Jid,
    pub author: Option<Jid>,
    pub action: GroupAction,
    pub participants: Vec<Jid>,
    pub timestamp: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatUpdate {
    Muted { chat: Jid, until: Option<i64> },
    Archived { chat: Jid, archived: bool },
    Pinned { chat: Jid, pinned: bool },
    MarkedRead { chat: Jid, read: bool },
    Locked { chat: Jid, locked: bool },
    Deleted { chat: Jid },
    Cleared { chat: Jid },
    Starred { chat: Jid, message_id: String, from_me: bool, starred: bool },
    DeletedForMe { chat: Jid, message_id: String, from_me: bool },
    Contact { jid: Jid, full_name: Option<String>, first_name: Option<String>, lid: Option<Jid>, phone: Option<Jid> },
    ContactRemoved { jid: Jid },
    PushName { name: String },
    Label { id: String, name: Option<String>, color: Option<i32>, deleted: bool },
    ChatLabel { label: String, chat: Jid, labeled: bool },
    MessageLabel { label: String, chat: Jid, message_id: String, labeled: bool },
    Other { index: Vec<String> },
}

#[derive(Clone, Debug)]
pub struct ServerAck {
    pub id: String,
    pub chat: Jid,
    pub error: Option<u16>,
    pub latency: Option<Duration>,
    pub timings: Option<crate::client::SendTimings>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResponseKind {
    Going,
    NotGoing,
    Maybe,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct EventResponse {
    pub chat: Jid,
    pub event_id: String,
    pub responder: Jid,
    pub kind: EventResponseKind,
    pub extra_guests: u32,
    pub timestamp: i64,
}

#[derive(Clone, Debug)]
pub struct PollVote {
    pub chat: Jid,
    pub poll_id: String,
    pub voter: Jid,
    pub selected: Vec<String>,
    pub timestamp: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    Offer,
    Accept,
    Reject,
    Terminate,
}

#[derive(Clone, Debug)]
pub struct CallEvent {
    pub id: String,
    pub from: Jid,
    pub kind: CallKind,
    pub video: bool,
    pub is_group: bool,
    pub timestamp: i64,
}
