use super::{Message, MessageKey};

#[derive(Clone, PartialEq, prost::Message)]
pub struct HistorySyncNotification {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "2")]
    pub file_length: Option<u64>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub media_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub file_enc_sha256: Option<Vec<u8>>,
    #[prost(string, optional, tag = "5")]
    pub direct_path: Option<String>,
    #[prost(int32, optional, tag = "6")]
    pub sync_type: Option<i32>,
    #[prost(uint32, optional, tag = "7")]
    pub chunk_order: Option<u32>,
    #[prost(string, optional, tag = "8")]
    pub original_message_id: Option<String>,
    #[prost(uint32, optional, tag = "9")]
    pub progress: Option<u32>,
    #[prost(int64, optional, tag = "10")]
    pub oldest_msg_in_chunk_timestamp_sec: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "11")]
    pub initial_hist_bootstrap_inline_payload: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HistorySync {
    #[prost(int32, tag = "1")]
    pub sync_type: i32,
    #[prost(message, repeated, tag = "2")]
    pub conversations: Vec<Conversation>,
    #[prost(message, repeated, tag = "3")]
    pub status_v3_messages: Vec<WebMessageInfo>,
    #[prost(uint32, optional, tag = "5")]
    pub chunk_order: Option<u32>,
    #[prost(uint32, optional, tag = "6")]
    pub progress: Option<u32>,
    #[prost(message, repeated, tag = "7")]
    pub pushnames: Vec<Pushname>,
    #[prost(message, repeated, tag = "15")]
    pub phone_number_to_lid_mappings: Vec<PhoneNumberToLidMapping>,
}

impl HistorySync {
    pub const INITIAL_BOOTSTRAP: i32 = 0;
    pub const INITIAL_STATUS_V3: i32 = 1;
    pub const FULL: i32 = 2;
    pub const RECENT: i32 = 3;
    pub const PUSH_NAME: i32 = 4;
    pub const NON_BLOCKING_DATA: i32 = 5;
    pub const ON_DEMAND: i32 = 6;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Conversation {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(message, repeated, tag = "2")]
    pub messages: Vec<HistorySyncMsg>,
    #[prost(string, optional, tag = "3")]
    pub new_jid: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub old_jid: Option<String>,
    #[prost(uint64, optional, tag = "5")]
    pub last_msg_timestamp: Option<u64>,
    #[prost(uint32, optional, tag = "6")]
    pub unread_count: Option<u32>,
    #[prost(bool, optional, tag = "7")]
    pub read_only: Option<bool>,
    #[prost(uint32, optional, tag = "9")]
    pub ephemeral_expiration: Option<u32>,
    #[prost(uint64, optional, tag = "12")]
    pub conversation_timestamp: Option<u64>,
    #[prost(string, optional, tag = "13")]
    pub name: Option<String>,
    #[prost(bool, optional, tag = "16")]
    pub archived: Option<bool>,
    #[prost(uint32, optional, tag = "24")]
    pub pinned: Option<u32>,
    #[prost(uint64, optional, tag = "25")]
    pub mute_end_time: Option<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HistorySyncMsg {
    #[prost(message, optional, tag = "1")]
    pub message: Option<WebMessageInfo>,
    #[prost(uint64, optional, tag = "2")]
    pub msg_order_id: Option<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct WebMessageInfo {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKey>,
    #[prost(message, optional, boxed, tag = "2")]
    pub message: Option<Box<Message>>,
    #[prost(uint64, optional, tag = "3")]
    pub message_timestamp: Option<u64>,
    #[prost(int32, optional, tag = "4")]
    pub status: Option<i32>,
    #[prost(string, optional, tag = "5")]
    pub participant: Option<String>,
    #[prost(bool, optional, tag = "17")]
    pub starred: Option<bool>,
    #[prost(bool, optional, tag = "18")]
    pub broadcast: Option<bool>,
    #[prost(string, optional, tag = "19")]
    pub push_name: Option<String>,
    #[prost(int32, optional, tag = "24")]
    pub message_stub_type: Option<i32>,
    #[prost(string, repeated, tag = "26")]
    pub message_stub_parameters: Vec<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Pushname {
    #[prost(string, optional, tag = "1")]
    pub id: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub pushname: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PhoneNumberToLidMapping {
    #[prost(string, optional, tag = "1")]
    pub pn_jid: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub lid_jid: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PollVoteMessage {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub selected_options: Vec<Vec<u8>>,
}
