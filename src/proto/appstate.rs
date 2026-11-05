use super::MessageKey;

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppStateSyncKeyShare {
    #[prost(message, repeated, tag = "1")]
    pub keys: Vec<AppStateSyncKey>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppStateSyncKey {
    #[prost(message, optional, tag = "1")]
    pub key_id: Option<AppStateSyncKeyId>,
    #[prost(message, optional, tag = "2")]
    pub key_data: Option<AppStateSyncKeyData>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppStateSyncKeyId {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub key_id: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppStateSyncKeyData {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub key_data: Option<Vec<u8>>,
    #[prost(int64, optional, tag = "3")]
    pub timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExternalBlobReference {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub media_key: Option<Vec<u8>>,
    #[prost(string, optional, tag = "2")]
    pub direct_path: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub handle: Option<String>,
    #[prost(uint64, optional, tag = "4")]
    pub file_size_bytes: Option<u64>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub file_sha256: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "6")]
    pub file_enc_sha256: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdVersion {
    #[prost(uint64, optional, tag = "1")]
    pub version: Option<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct KeyId {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub id: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdBlob {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub blob: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdRecord {
    #[prost(message, optional, tag = "1")]
    pub index: Option<SyncdBlob>,
    #[prost(message, optional, tag = "2")]
    pub value: Option<SyncdBlob>,
    #[prost(message, optional, tag = "3")]
    pub key_id: Option<KeyId>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdMutation {
    #[prost(int32, optional, tag = "1")]
    pub operation: Option<i32>,
    #[prost(message, optional, tag = "2")]
    pub record: Option<SyncdRecord>,
}

impl SyncdMutation {
    pub const SET: i32 = 0;
    pub const REMOVE: i32 = 1;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdMutations {
    #[prost(message, repeated, tag = "1")]
    pub mutations: Vec<SyncdMutation>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdPatch {
    #[prost(message, optional, tag = "1")]
    pub version: Option<SyncdVersion>,
    #[prost(message, repeated, tag = "2")]
    pub mutations: Vec<SyncdMutation>,
    #[prost(message, optional, tag = "3")]
    pub external_mutations: Option<ExternalBlobReference>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub snapshot_mac: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub patch_mac: Option<Vec<u8>>,
    #[prost(message, optional, tag = "6")]
    pub key_id: Option<KeyId>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncdSnapshot {
    #[prost(message, optional, tag = "1")]
    pub version: Option<SyncdVersion>,
    #[prost(message, repeated, tag = "2")]
    pub records: Vec<SyncdRecord>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub mac: Option<Vec<u8>>,
    #[prost(message, optional, tag = "4")]
    pub key_id: Option<KeyId>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncActionData {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub index: Option<Vec<u8>>,
    #[prost(message, optional, tag = "2")]
    pub value: Option<SyncActionValue>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub padding: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "4")]
    pub version: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncActionValue {
    #[prost(int64, optional, tag = "1")]
    pub timestamp: Option<i64>,
    #[prost(message, optional, tag = "2")]
    pub star_action: Option<StarAction>,
    #[prost(message, optional, tag = "3")]
    pub contact_action: Option<ContactAction>,
    #[prost(message, optional, tag = "4")]
    pub mute_action: Option<MuteAction>,
    #[prost(message, optional, tag = "5")]
    pub pin_action: Option<PinAction>,
    #[prost(message, optional, tag = "7")]
    pub push_name_setting: Option<PushNameSetting>,
    #[prost(message, optional, tag = "8")]
    pub quick_reply_action: Option<QuickReplyAction>,
    #[prost(message, optional, tag = "14")]
    pub label_edit_action: Option<LabelEditAction>,
    #[prost(message, optional, tag = "15")]
    pub label_association_action: Option<LabelAssociationAction>,
    #[prost(message, optional, tag = "17")]
    pub archive_chat_action: Option<ArchiveChatAction>,
    #[prost(message, optional, tag = "18")]
    pub delete_message_for_me_action: Option<DeleteMessageForMeAction>,
    #[prost(message, optional, tag = "20")]
    pub mark_chat_as_read_action: Option<MarkChatAsReadAction>,
    #[prost(message, optional, tag = "21")]
    pub clear_chat_action: Option<MessageRangeAction>,
    #[prost(message, optional, tag = "22")]
    pub delete_chat_action: Option<MessageRangeAction>,
    #[prost(message, optional, tag = "50")]
    pub lock_chat_action: Option<LockChatAction>,
    #[prost(message, optional, tag = "53")]
    pub privacy_setting_disable_link_previews_action: Option<PrivacySettingDisableLinkPreviewsAction>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct QuickReplyAction {
    #[prost(string, optional, tag = "1")]
    pub shortcut: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub message: Option<String>,
    #[prost(string, repeated, tag = "3")]
    pub keywords: Vec<String>,
    #[prost(int32, optional, tag = "4")]
    pub count: Option<i32>,
    #[prost(bool, optional, tag = "5")]
    pub deleted: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PrivacySettingDisableLinkPreviewsAction {
    #[prost(bool, optional, tag = "1")]
    pub is_previews_disabled: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StarAction {
    #[prost(bool, optional, tag = "1")]
    pub starred: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ContactAction {
    #[prost(string, optional, tag = "1")]
    pub full_name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub first_name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub lid_jid: Option<String>,
    #[prost(bool, optional, tag = "4")]
    pub save_on_primary_addressbook: Option<bool>,
    #[prost(string, optional, tag = "5")]
    pub pn_jid: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub username: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MuteAction {
    #[prost(bool, optional, tag = "1")]
    pub muted: Option<bool>,
    #[prost(int64, optional, tag = "2")]
    pub mute_end_timestamp: Option<i64>,
    #[prost(bool, optional, tag = "3")]
    pub auto_muted: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PinAction {
    #[prost(bool, optional, tag = "1")]
    pub pinned: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PushNameSetting {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LabelEditAction {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub color: Option<i32>,
    #[prost(int32, optional, tag = "3")]
    pub predefined_id: Option<i32>,
    #[prost(bool, optional, tag = "4")]
    pub deleted: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LabelAssociationAction {
    #[prost(bool, optional, tag = "1")]
    pub labeled: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ArchiveChatAction {
    #[prost(bool, optional, tag = "1")]
    pub archived: Option<bool>,
    #[prost(message, optional, tag = "2")]
    pub message_range: Option<SyncActionMessageRange>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DeleteMessageForMeAction {
    #[prost(bool, optional, tag = "1")]
    pub delete_media: Option<bool>,
    #[prost(int64, optional, tag = "2")]
    pub message_timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MarkChatAsReadAction {
    #[prost(bool, optional, tag = "1")]
    pub read: Option<bool>,
    #[prost(message, optional, tag = "2")]
    pub message_range: Option<SyncActionMessageRange>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MessageRangeAction {
    #[prost(message, optional, tag = "1")]
    pub message_range: Option<SyncActionMessageRange>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LockChatAction {
    #[prost(bool, optional, tag = "1")]
    pub locked: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncActionMessageRange {
    #[prost(int64, optional, tag = "1")]
    pub last_message_timestamp: Option<i64>,
    #[prost(int64, optional, tag = "2")]
    pub last_system_message_timestamp: Option<i64>,
    #[prost(message, repeated, tag = "3")]
    pub messages: Vec<SyncActionMessage>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SyncActionMessage {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKey>,
    #[prost(int64, optional, tag = "2")]
    pub timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredHashEntry {
    #[prost(bytes = "vec", tag = "1")]
    pub index_mac: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value_mac: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredHashState {
    #[prost(uint64, tag = "1")]
    pub version: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub hash: Vec<u8>,
    #[prost(message, repeated, tag = "3")]
    pub entries: Vec<StoredHashEntry>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredTcToken {
    #[prost(bytes = "vec", tag = "1")]
    pub token: Vec<u8>,
    #[prost(int64, tag = "2")]
    pub timestamp: i64,
    #[prost(int64, optional, tag = "3")]
    pub sender_timestamp: Option<i64>,
}
