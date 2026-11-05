use super::{AppStateSyncKeyId, MessageKey};

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppStateSyncKeyRequest {
    #[prost(message, repeated, tag = "1")]
    pub key_ids: Vec<AppStateSyncKeyId>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PeerDataOperationRequestMessage {
    #[prost(int32, optional, tag = "1")]
    pub request_type: Option<i32>,
    #[prost(message, optional, tag = "4")]
    pub history_sync_on_demand_request: Option<HistorySyncOnDemandRequest>,
    #[prost(message, repeated, tag = "5")]
    pub placeholder_message_resend_request: Vec<PlaceholderMessageResendRequest>,
}

impl PeerDataOperationRequestMessage {
    pub const HISTORY_SYNC_ON_DEMAND: i32 = 3;
    pub const PLACEHOLDER_MESSAGE_RESEND: i32 = 4;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HistorySyncOnDemandRequest {
    #[prost(string, optional, tag = "1")]
    pub chat_jid: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub oldest_msg_id: Option<String>,
    #[prost(bool, optional, tag = "3")]
    pub oldest_msg_from_me: Option<bool>,
    #[prost(int32, optional, tag = "4")]
    pub on_demand_msg_count: Option<i32>,
    #[prost(int64, optional, tag = "5")]
    pub oldest_msg_timestamp_ms: Option<i64>,
    #[prost(string, optional, tag = "6")]
    pub account_lid: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PlaceholderMessageResendRequest {
    #[prost(message, optional, tag = "1")]
    pub message_key: Option<MessageKey>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PeerDataOperationRequestResponseMessage {
    #[prost(int32, optional, tag = "1")]
    pub request_type: Option<i32>,
    #[prost(string, optional, tag = "2")]
    pub stanza_id: Option<String>,
    #[prost(message, repeated, tag = "3")]
    pub results: Vec<PeerDataOperationResult>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PeerDataOperationResult {
    #[prost(message, optional, tag = "4")]
    pub placeholder_message_resend_response: Option<PlaceholderMessageResendResponse>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PlaceholderMessageResendResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub web_message_info_bytes: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ServerErrorReceipt {
    #[prost(string, optional, tag = "1")]
    pub stanza_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MediaRetryNotification {
    #[prost(string, optional, tag = "1")]
    pub stanza_id: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub direct_path: Option<String>,
    #[prost(int32, optional, tag = "3")]
    pub result: Option<i32>,
}

impl MediaRetryNotification {
    pub const SUCCESS: i32 = 1;
}
