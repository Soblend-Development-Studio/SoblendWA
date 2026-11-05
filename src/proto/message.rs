use super::{
    ButtonsMessage, ButtonsResponseMessage, InteractiveMessage, InteractiveResponseMessage, ListMessage,
    ListResponseMessage, TemplateButtonReplyMessage, TemplateMessage,
};

#[derive(Clone, PartialEq, prost::Message)]
pub struct Message {
    #[prost(string, optional, tag = "1")]
    pub conversation: Option<String>,
    #[prost(message, optional, boxed, tag = "2")]
    pub sender_key_distribution_message: Option<Box<SenderKeyDistributionMessage>>,
    #[prost(message, optional, boxed, tag = "3")]
    pub image_message: Option<Box<ImageMessage>>,
    #[prost(message, optional, boxed, tag = "4")]
    pub contact_message: Option<Box<ContactMessage>>,
    #[prost(message, optional, boxed, tag = "5")]
    pub location_message: Option<Box<LocationMessage>>,
    #[prost(message, optional, boxed, tag = "6")]
    pub extended_text_message: Option<Box<ExtendedTextMessage>>,
    #[prost(message, optional, boxed, tag = "7")]
    pub document_message: Option<Box<DocumentMessage>>,
    #[prost(message, optional, boxed, tag = "8")]
    pub audio_message: Option<Box<AudioMessage>>,
    #[prost(message, optional, boxed, tag = "9")]
    pub video_message: Option<Box<VideoMessage>>,
    #[prost(message, optional, boxed, tag = "12")]
    pub protocol_message: Option<Box<ProtocolMessage>>,
    #[prost(message, optional, boxed, tag = "13")]
    pub contacts_array_message: Option<Box<ContactsArrayMessage>>,
    #[prost(message, optional, boxed, tag = "25")]
    pub template_message: Option<Box<TemplateMessage>>,
    #[prost(message, optional, boxed, tag = "26")]
    pub sticker_message: Option<Box<StickerMessage>>,
    #[prost(message, optional, boxed, tag = "28")]
    pub group_invite_message: Option<Box<GroupInviteMessage>>,
    #[prost(message, optional, boxed, tag = "29")]
    pub template_button_reply_message: Option<Box<TemplateButtonReplyMessage>>,
    #[prost(message, optional, boxed, tag = "30")]
    pub product_message: Option<Box<ProductMessage>>,
    #[prost(message, optional, boxed, tag = "31")]
    pub device_sent_message: Option<Box<DeviceSentMessage>>,
    #[prost(message, optional, boxed, tag = "35")]
    pub message_context_info: Option<Box<MessageContextInfo>>,
    #[prost(message, optional, boxed, tag = "36")]
    pub list_message: Option<Box<ListMessage>>,
    #[prost(message, optional, boxed, tag = "37")]
    pub view_once_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "39")]
    pub list_response_message: Option<Box<ListResponseMessage>>,
    #[prost(message, optional, boxed, tag = "40")]
    pub ephemeral_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "42")]
    pub buttons_message: Option<Box<ButtonsMessage>>,
    #[prost(message, optional, boxed, tag = "43")]
    pub buttons_response_message: Option<Box<ButtonsResponseMessage>>,
    #[prost(message, optional, boxed, tag = "45")]
    pub interactive_message: Option<Box<InteractiveMessage>>,
    #[prost(message, optional, boxed, tag = "46")]
    pub reaction_message: Option<Box<ReactionMessage>>,
    #[prost(message, optional, boxed, tag = "48")]
    pub interactive_response_message: Option<Box<InteractiveResponseMessage>>,
    #[prost(message, optional, boxed, tag = "49")]
    pub poll_creation_message: Option<Box<PollCreationMessage>>,
    #[prost(message, optional, boxed, tag = "50")]
    pub poll_update_message: Option<Box<PollUpdateMessage>>,
    #[prost(message, optional, boxed, tag = "53")]
    pub document_with_caption_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "54")]
    pub request_phone_number_message: Option<Box<RequestPhoneNumberMessage>>,
    #[prost(message, optional, boxed, tag = "55")]
    pub view_once_message_v2: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "58")]
    pub edited_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "59")]
    pub view_once_message_v2_extension: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "62")]
    pub group_mentioned_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "67")]
    pub bot_invoke_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "74")]
    pub lottie_sticker_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "87")]
    pub status_mention_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "91")]
    pub associated_child_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "92")]
    pub group_status_mention_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "93")]
    pub poll_creation_message_v4: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "96")]
    pub group_status_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "100")]
    pub bot_task_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "101")]
    pub question_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "103")]
    pub group_status_message_v2: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "104")]
    pub bot_forwarded_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "106")]
    pub question_reply_message: Option<Box<FutureProofMessage>>,
    #[prost(message, optional, boxed, tag = "60")]
    pub poll_creation_message_v2: Option<Box<PollCreationMessage>>,
    #[prost(message, optional, boxed, tag = "63")]
    pub pin_in_chat_message: Option<Box<PinInChatMessage>>,
    #[prost(message, optional, boxed, tag = "64")]
    pub poll_creation_message_v3: Option<Box<PollCreationMessage>>,
    #[prost(message, optional, boxed, tag = "66")]
    pub ptv_message: Option<Box<VideoMessage>>,
    #[prost(message, optional, boxed, tag = "75")]
    pub event_message: Option<Box<EventMessage>>,
    #[prost(message, optional, boxed, tag = "76")]
    pub enc_event_response_message: Option<Box<EncEventResponseMessage>>,
    #[prost(message, optional, boxed, tag = "83")]
    pub album_message: Option<Box<AlbumMessage>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MessageKey {
    #[prost(string, optional, tag = "1")]
    pub remote_jid: Option<String>,
    #[prost(bool, optional, tag = "2")]
    pub from_me: Option<bool>,
    #[prost(string, optional, tag = "3")]
    pub id: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub participant: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ContextInfo {
    #[prost(string, optional, tag = "1")]
    pub stanza_id: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub participant: Option<String>,
    #[prost(message, optional, boxed, tag = "3")]
    pub quoted_message: Option<Box<Message>>,
    #[prost(string, optional, tag = "4")]
    pub remote_jid: Option<String>,
    #[prost(string, repeated, tag = "15")]
    pub mentioned_jid: Vec<String>,
    #[prost(uint32, optional, tag = "21")]
    pub forwarding_score: Option<u32>,
    #[prost(bool, optional, tag = "22")]
    pub is_forwarded: Option<bool>,
    #[prost(uint32, optional, tag = "25")]
    pub expiration: Option<u32>,
    #[prost(int64, optional, tag = "26")]
    pub ephemeral_setting_timestamp: Option<i64>,
    #[prost(uint32, optional, tag = "70")]
    pub non_jid_mentions: Option<u32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MessageContextInfo {
    #[prost(int32, optional, tag = "2")]
    pub device_list_metadata_version: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub message_secret: Option<Vec<u8>>,
    #[prost(uint32, optional, tag = "5")]
    pub message_add_on_duration_in_secs: Option<u32>,
    #[prost(message, optional, tag = "10")]
    pub message_association: Option<MessageAssociation>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderKeyDistributionMessage {
    #[prost(string, optional, tag = "1")]
    pub group_id: Option<String>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub axolotl_sender_key_distribution_message: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExtendedTextMessage {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub matched_text: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub description: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub title: Option<String>,
    #[prost(int32, optional, tag = "10")]
    pub preview_type: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "16")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
    #[prost(string, optional, tag = "19")]
    pub thumbnail_direct_path: Option<String>,
    #[prost(bytes = "vec", optional, tag = "20")]
    pub thumbnail_sha256: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "21")]
    pub thumbnail_enc_sha256: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "22")]
    pub media_key: Option<Vec<u8>>,
    #[prost(int64, optional, tag = "23")]
    pub media_key_timestamp: Option<i64>,
    #[prost(uint32, optional, tag = "24")]
    pub thumbnail_height: Option<u32>,
    #[prost(uint32, optional, tag = "25")]
    pub thumbnail_width: Option<u32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ImageMessage {
    #[prost(string, optional, tag = "1")]
    pub url: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub mimetype: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub caption: Option<String>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "5")]
    pub file_length: Option<u64>,
    #[prost(uint32, optional, tag = "6")]
    pub height: Option<u32>,
    #[prost(uint32, optional, tag = "7")]
    pub width: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "8")]
    pub media_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "9")]
    pub file_enc_sha256: Option<Vec<u8>>,
    #[prost(string, optional, tag = "11")]
    pub direct_path: Option<String>,
    #[prost(int64, optional, tag = "12")]
    pub media_key_timestamp: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "16")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
    #[prost(bool, optional, tag = "25")]
    pub view_once: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct VideoMessage {
    #[prost(string, optional, tag = "1")]
    pub url: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub mimetype: Option<String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "4")]
    pub file_length: Option<u64>,
    #[prost(uint32, optional, tag = "5")]
    pub seconds: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "6")]
    pub media_key: Option<Vec<u8>>,
    #[prost(string, optional, tag = "7")]
    pub caption: Option<String>,
    #[prost(bool, optional, tag = "8")]
    pub gif_playback: Option<bool>,
    #[prost(uint32, optional, tag = "9")]
    pub height: Option<u32>,
    #[prost(uint32, optional, tag = "10")]
    pub width: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "11")]
    pub file_enc_sha256: Option<Vec<u8>>,
    #[prost(string, optional, tag = "13")]
    pub direct_path: Option<String>,
    #[prost(int64, optional, tag = "14")]
    pub media_key_timestamp: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "16")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
    #[prost(bool, optional, tag = "20")]
    pub view_once: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AudioMessage {
    #[prost(string, optional, tag = "1")]
    pub url: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub mimetype: Option<String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "4")]
    pub file_length: Option<u64>,
    #[prost(uint32, optional, tag = "5")]
    pub seconds: Option<u32>,
    #[prost(bool, optional, tag = "6")]
    pub ptt: Option<bool>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub media_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    pub file_enc_sha256: Option<Vec<u8>>,
    #[prost(string, optional, tag = "9")]
    pub direct_path: Option<String>,
    #[prost(int64, optional, tag = "10")]
    pub media_key_timestamp: Option<i64>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
    #[prost(bytes = "vec", optional, tag = "19")]
    pub waveform: Option<Vec<u8>>,
    #[prost(bool, optional, tag = "21")]
    pub view_once: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DocumentMessage {
    #[prost(string, optional, tag = "1")]
    pub url: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub mimetype: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub title: Option<String>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "5")]
    pub file_length: Option<u64>,
    #[prost(uint32, optional, tag = "6")]
    pub page_count: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub media_key: Option<Vec<u8>>,
    #[prost(string, optional, tag = "8")]
    pub file_name: Option<String>,
    #[prost(bytes = "vec", optional, tag = "9")]
    pub file_enc_sha256: Option<Vec<u8>>,
    #[prost(string, optional, tag = "10")]
    pub direct_path: Option<String>,
    #[prost(int64, optional, tag = "11")]
    pub media_key_timestamp: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "16")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
    #[prost(string, optional, tag = "20")]
    pub caption: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StickerMessage {
    #[prost(string, optional, tag = "1")]
    pub url: Option<String>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub file_enc_sha256: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub media_key: Option<Vec<u8>>,
    #[prost(string, optional, tag = "5")]
    pub mimetype: Option<String>,
    #[prost(uint32, optional, tag = "6")]
    pub height: Option<u32>,
    #[prost(uint32, optional, tag = "7")]
    pub width: Option<u32>,
    #[prost(string, optional, tag = "8")]
    pub direct_path: Option<String>,
    #[prost(uint64, optional, tag = "9")]
    pub file_length: Option<u64>,
    #[prost(int64, optional, tag = "10")]
    pub media_key_timestamp: Option<i64>,
    #[prost(bool, optional, tag = "13")]
    pub is_animated: Option<bool>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LocationMessage {
    #[prost(double, optional, tag = "1")]
    pub degrees_latitude: Option<f64>,
    #[prost(double, optional, tag = "2")]
    pub degrees_longitude: Option<f64>,
    #[prost(string, optional, tag = "3")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub address: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub url: Option<String>,
    #[prost(bool, optional, tag = "6")]
    pub is_live: Option<bool>,
    #[prost(string, optional, tag = "11")]
    pub comment: Option<String>,
    #[prost(bytes = "vec", optional, tag = "16")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ContactMessage {
    #[prost(string, optional, tag = "1")]
    pub display_name: Option<String>,
    #[prost(string, optional, tag = "16")]
    pub vcard: Option<String>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ContactsArrayMessage {
    #[prost(string, optional, tag = "1")]
    pub display_name: Option<String>,
    #[prost(message, repeated, tag = "2")]
    pub contacts: Vec<ContactMessage>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GroupInviteMessage {
    #[prost(string, optional, tag = "1")]
    pub group_jid: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub invite_code: Option<String>,
    #[prost(int64, optional, tag = "3")]
    pub invite_expiration: Option<i64>,
    #[prost(string, optional, tag = "4")]
    pub group_name: Option<String>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(string, optional, tag = "6")]
    pub caption: Option<String>,
    #[prost(message, optional, tag = "7")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DeviceSentMessage {
    #[prost(string, optional, tag = "1")]
    pub destination_jid: Option<String>,
    #[prost(message, optional, boxed, tag = "2")]
    pub message: Option<Box<Message>>,
    #[prost(string, optional, tag = "3")]
    pub phash: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FutureProofMessage {
    #[prost(message, optional, boxed, tag = "1")]
    pub message: Option<Box<Message>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ProtocolMessage {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKey>,
    #[prost(int32, optional, tag = "2")]
    pub r#type: Option<i32>,
    #[prost(uint32, optional, tag = "4")]
    pub ephemeral_expiration: Option<u32>,
    #[prost(int64, optional, tag = "5")]
    pub ephemeral_setting_timestamp: Option<i64>,
    #[prost(message, optional, tag = "6")]
    pub history_sync_notification: Option<super::HistorySyncNotification>,
    #[prost(message, optional, tag = "7")]
    pub app_state_sync_key_share: Option<super::AppStateSyncKeyShare>,
    #[prost(message, optional, tag = "8")]
    pub app_state_sync_key_request: Option<super::AppStateSyncKeyRequest>,
    #[prost(message, optional, boxed, tag = "14")]
    pub edited_message: Option<Box<Message>>,
    #[prost(int64, optional, tag = "15")]
    pub timestamp_ms: Option<i64>,
    #[prost(message, optional, tag = "16")]
    pub peer_data_operation_request_message: Option<super::PeerDataOperationRequestMessage>,
    #[prost(message, optional, tag = "17")]
    pub peer_data_operation_request_response_message: Option<super::PeerDataOperationRequestResponseMessage>,
    #[prost(message, optional, tag = "24")]
    pub limit_sharing: Option<LimitSharing>,
    #[prost(message, optional, tag = "27")]
    pub member_label: Option<MemberLabel>,
}

impl ProtocolMessage {
    pub const REVOKE: i32 = 0;
    pub const EPHEMERAL_SETTING: i32 = 3;
    pub const HISTORY_SYNC_NOTIFICATION: i32 = 5;
    pub const APP_STATE_SYNC_KEY_SHARE: i32 = 6;
    pub const APP_STATE_SYNC_KEY_REQUEST: i32 = 7;
    pub const MESSAGE_EDIT: i32 = 14;
    pub const PEER_DATA_OPERATION_REQUEST: i32 = 16;
    pub const PEER_DATA_OPERATION_RESPONSE: i32 = 17;
    pub const SHARE_PHONE_NUMBER: i32 = 11;
    pub const LIMIT_SHARING: i32 = 27;
    pub const GROUP_MEMBER_LABEL_CHANGE: i32 = 30;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LimitSharing {
    #[prost(bool, optional, tag = "1")]
    pub sharing_limited: Option<bool>,
    #[prost(int32, optional, tag = "2")]
    pub trigger: Option<i32>,
    #[prost(int64, optional, tag = "3")]
    pub limit_sharing_setting_timestamp: Option<i64>,
    #[prost(bool, optional, tag = "4")]
    pub initiated_by_me: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MemberLabel {
    #[prost(string, optional, tag = "1")]
    pub label: Option<String>,
    #[prost(int64, optional, tag = "2")]
    pub label_timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct RequestPhoneNumberMessage {
    #[prost(message, optional, tag = "1")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ProductMessage {
    #[prost(message, optional, tag = "1")]
    pub product: Option<ProductSnapshot>,
    #[prost(string, optional, tag = "2")]
    pub business_owner_jid: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub body: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub footer: Option<String>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ProductSnapshot {
    #[prost(message, optional, tag = "1")]
    pub product_image: Option<ImageMessage>,
    #[prost(string, optional, tag = "2")]
    pub product_id: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub title: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub description: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub currency_code: Option<String>,
    #[prost(int64, optional, tag = "6")]
    pub price_amount_1000: Option<i64>,
    #[prost(string, optional, tag = "7")]
    pub retailer_id: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub url: Option<String>,
    #[prost(uint32, optional, tag = "9")]
    pub product_image_count: Option<u32>,
    #[prost(int64, optional, tag = "12")]
    pub sale_price_amount_1000: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ReactionMessage {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKey>,
    #[prost(string, optional, tag = "2")]
    pub text: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub grouping_key: Option<String>,
    #[prost(int64, optional, tag = "4")]
    pub sender_timestamp_ms: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PollCreationMessage {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub enc_key: Option<Vec<u8>>,
    #[prost(string, optional, tag = "2")]
    pub name: Option<String>,
    #[prost(message, repeated, tag = "3")]
    pub options: Vec<PollOption>,
    #[prost(uint32, optional, tag = "4")]
    pub selectable_options_count: Option<u32>,
    #[prost(message, optional, tag = "5")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PollOption {
    #[prost(string, optional, tag = "1")]
    pub option_name: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PollUpdateMessage {
    #[prost(message, optional, tag = "1")]
    pub poll_creation_message_key: Option<MessageKey>,
    #[prost(message, optional, tag = "2")]
    pub vote: Option<PollEncValue>,
    #[prost(int64, optional, tag = "4")]
    pub sender_timestamp_ms: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PollEncValue {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub enc_payload: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub enc_iv: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PinInChatMessage {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKey>,
    #[prost(int32, optional, tag = "2")]
    pub r#type: Option<i32>,
    #[prost(int64, optional, tag = "3")]
    pub sender_timestamp_ms: Option<i64>,
}

impl PinInChatMessage {
    pub const PIN_FOR_ALL: i32 = 1;
    pub const UNPIN_FOR_ALL: i32 = 2;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct EventMessage {
    #[prost(message, optional, tag = "1")]
    pub context_info: Option<ContextInfo>,
    #[prost(bool, optional, tag = "2")]
    pub is_canceled: Option<bool>,
    #[prost(string, optional, tag = "3")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub description: Option<String>,
    #[prost(message, optional, tag = "5")]
    pub location: Option<LocationMessage>,
    #[prost(string, optional, tag = "6")]
    pub join_link: Option<String>,
    #[prost(int64, optional, tag = "7")]
    pub start_time: Option<i64>,
    #[prost(int64, optional, tag = "8")]
    pub end_time: Option<i64>,
    #[prost(bool, optional, tag = "9")]
    pub extra_guests_allowed: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct EncEventResponseMessage {
    #[prost(message, optional, tag = "1")]
    pub event_creation_message_key: Option<MessageKey>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub enc_payload: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub enc_iv: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct EventResponseMessage {
    #[prost(int32, optional, tag = "1")]
    pub response: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    pub timestamp_ms: Option<i64>,
    #[prost(int32, optional, tag = "3")]
    pub extra_guest_count: Option<i32>,
}

impl EventResponseMessage {
    pub const GOING: i32 = 1;
    pub const NOT_GOING: i32 = 2;
    pub const MAYBE: i32 = 3;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AlbumMessage {
    #[prost(uint32, optional, tag = "2")]
    pub expected_image_count: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub expected_video_count: Option<u32>,
    #[prost(message, optional, tag = "17")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MessageAssociation {
    #[prost(int32, optional, tag = "1")]
    pub association_type: Option<i32>,
    #[prost(message, optional, tag = "2")]
    pub parent_message_key: Option<MessageKey>,
    #[prost(int32, optional, tag = "3")]
    pub message_index: Option<i32>,
}

impl MessageAssociation {
    pub const MEDIA_ALBUM: i32 = 1;
}
