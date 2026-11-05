#[derive(Clone, PartialEq, prost::Message)]
pub struct HandshakeMessage {
    #[prost(message, optional, tag = "2")]
    pub client_hello: Option<ClientHello>,
    #[prost(message, optional, tag = "3")]
    pub server_hello: Option<ServerHello>,
    #[prost(message, optional, tag = "4")]
    pub client_finish: Option<ClientFinish>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ClientHello {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub ephemeral: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub static_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub payload: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ServerHello {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub ephemeral: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub static_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub payload: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ClientFinish {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub static_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub payload: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CertChain {
    #[prost(message, optional, tag = "1")]
    pub leaf: Option<NoiseCertificate>,
    #[prost(message, optional, tag = "2")]
    pub intermediate: Option<NoiseCertificate>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NoiseCertificate {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub details: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub signature: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CertificateDetails {
    #[prost(uint32, optional, tag = "1")]
    pub serial: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub issuer_serial: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub key: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "4")]
    pub not_before: Option<u64>,
    #[prost(uint64, optional, tag = "5")]
    pub not_after: Option<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ClientPayload {
    #[prost(uint64, optional, tag = "1")]
    pub username: Option<u64>,
    #[prost(bool, optional, tag = "3")]
    pub passive: Option<bool>,
    #[prost(message, optional, tag = "5")]
    pub user_agent: Option<UserAgent>,
    #[prost(message, optional, tag = "6")]
    pub web_info: Option<WebInfo>,
    #[prost(string, optional, tag = "7")]
    pub push_name: Option<String>,
    #[prost(int32, optional, tag = "12")]
    pub connect_type: Option<i32>,
    #[prost(int32, optional, tag = "13")]
    pub connect_reason: Option<i32>,
    #[prost(uint32, optional, tag = "18")]
    pub device: Option<u32>,
    #[prost(message, optional, tag = "19")]
    pub device_pairing_data: Option<DevicePairingRegistrationData>,
    #[prost(bool, optional, tag = "33")]
    pub pull: Option<bool>,
    #[prost(bool, optional, tag = "41")]
    pub lid_db_migrated: Option<bool>,
}

impl ClientPayload {
    pub const CONNECT_TYPE_WIFI_UNKNOWN: i32 = 1;
    pub const CONNECT_REASON_USER_ACTIVATED: i32 = 1;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct UserAgent {
    #[prost(int32, optional, tag = "1")]
    pub platform: Option<i32>,
    #[prost(message, optional, tag = "2")]
    pub app_version: Option<AppVersion>,
    #[prost(string, optional, tag = "3")]
    pub mcc: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub mnc: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub os_version: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub manufacturer: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub device: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub os_build_number: Option<String>,
    #[prost(int32, optional, tag = "10")]
    pub release_channel: Option<i32>,
    #[prost(string, optional, tag = "11")]
    pub locale_language_iso_639_1: Option<String>,
    #[prost(string, optional, tag = "12")]
    pub locale_country_iso_3166_1_alpha_2: Option<String>,
}

impl UserAgent {
    pub const PLATFORM_WEB: i32 = 14;
    pub const RELEASE_CHANNEL_RELEASE: i32 = 0;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppVersion {
    #[prost(uint32, optional, tag = "1")]
    pub primary: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub secondary: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub tertiary: Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    pub quaternary: Option<u32>,
    #[prost(uint32, optional, tag = "5")]
    pub quinary: Option<u32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct WebInfo {
    #[prost(string, optional, tag = "1")]
    pub ref_token: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub version: Option<String>,
    #[prost(int32, optional, tag = "4")]
    pub web_sub_platform: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DevicePairingRegistrationData {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub e_regid: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub e_keytype: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub e_ident: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub e_skey_id: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub e_skey_val: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "6")]
    pub e_skey_sig: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub build_hash: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    pub device_props: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DeviceProps {
    #[prost(string, optional, tag = "1")]
    pub os: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub version: Option<AppVersion>,
    #[prost(int32, optional, tag = "3")]
    pub platform_type: Option<i32>,
    #[prost(bool, optional, tag = "4")]
    pub require_full_sync: Option<bool>,
    #[prost(message, optional, tag = "5")]
    pub history_sync_config: Option<HistorySyncConfig>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HistorySyncConfig {
    #[prost(uint32, optional, tag = "1")]
    pub full_sync_days_limit: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub full_sync_size_mb_limit: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub storage_quota_mb: Option<u32>,
    #[prost(bool, optional, tag = "4")]
    pub inline_initial_payload_in_e2ee_msg: Option<bool>,
    #[prost(uint32, optional, tag = "5")]
    pub recent_sync_days_limit: Option<u32>,
    #[prost(bool, optional, tag = "6")]
    pub support_call_log_history: Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub support_bot_user_agent_chat_history: Option<bool>,
    #[prost(bool, optional, tag = "8")]
    pub support_cag_reactions_and_polls: Option<bool>,
    #[prost(bool, optional, tag = "9")]
    pub support_biz_hosted_msg: Option<bool>,
    #[prost(bool, optional, tag = "10")]
    pub support_recent_sync_chunk_message_count_tuning: Option<bool>,
    #[prost(bool, optional, tag = "11")]
    pub support_hosted_group_msg: Option<bool>,
    #[prost(bool, optional, tag = "12")]
    pub support_fbid_bot_chat_history: Option<bool>,
    #[prost(bool, optional, tag = "14")]
    pub support_message_association: Option<bool>,
    #[prost(bool, optional, tag = "15")]
    pub support_group_history: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AdvSignedDeviceIdentityHmac {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub details: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub hmac: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "3")]
    pub account_type: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AdvSignedDeviceIdentity {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub details: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub account_signature_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub account_signature: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub device_signature: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AdvDeviceIdentity {
    #[prost(uint32, optional, tag = "1")]
    pub raw_id: Option<u32>,
    #[prost(uint64, optional, tag = "2")]
    pub timestamp: Option<u64>,
    #[prost(uint32, optional, tag = "3")]
    pub key_index: Option<u32>,
    #[prost(int32, optional, tag = "4")]
    pub account_type: Option<i32>,
    #[prost(int32, optional, tag = "5")]
    pub device_type: Option<i32>,
}

pub const ADV_ENCRYPTION_HOSTED: i32 = 1;
