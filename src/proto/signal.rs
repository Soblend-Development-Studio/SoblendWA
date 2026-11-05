#[derive(Clone, PartialEq, prost::Message)]
pub struct WhisperMessage {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub ephemeral_key: Option<Vec<u8>>,
    #[prost(uint32, optional, tag = "2")]
    pub counter: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub previous_counter: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub ciphertext: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PreKeyWhisperMessage {
    #[prost(uint32, optional, tag = "1")]
    pub pre_key_id: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub base_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub identity_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub message: Option<Vec<u8>>,
    #[prost(uint32, optional, tag = "5")]
    pub registration_id: Option<u32>,
    #[prost(uint32, optional, tag = "6")]
    pub signed_pre_key_id: Option<u32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderKeyMessage {
    #[prost(uint32, optional, tag = "1")]
    pub id: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub iteration: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub ciphertext: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderKeyDistribution {
    #[prost(uint32, optional, tag = "1")]
    pub id: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub iteration: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub chain_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub signing_key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredMessageKey {
    #[prost(uint32, tag = "1")]
    pub index: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub key: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredChain {
    #[prost(bytes = "vec", tag = "1")]
    pub ratchet_key: Vec<u8>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub chain_key: Option<Vec<u8>>,
    #[prost(int64, tag = "3")]
    pub counter: i64,
    #[prost(message, repeated, tag = "4")]
    pub message_keys: Vec<StoredMessageKey>,
    #[prost(message, repeated, tag = "5")]
    pub derived_keys: Vec<StoredDerivedKey>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredDerivedKey {
    #[prost(uint32, tag = "1")]
    pub index: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub cipher_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub mac_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub iv: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredPendingPreKey {
    #[prost(uint32, optional, tag = "1")]
    pub pre_key_id: Option<u32>,
    #[prost(uint32, tag = "2")]
    pub signed_pre_key_id: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub base_key: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredSessionState {
    #[prost(uint32, tag = "1")]
    pub registration_id: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub root_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub our_ratchet_private: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub their_ratchet: Vec<u8>,
    #[prost(uint32, tag = "5")]
    pub previous_counter: u32,
    #[prost(message, optional, tag = "6")]
    pub sending: Option<StoredChain>,
    #[prost(message, repeated, tag = "7")]
    pub receiving: Vec<StoredChain>,
    #[prost(bytes = "vec", tag = "8")]
    pub remote_identity: Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub base_key: Vec<u8>,
    #[prost(message, optional, tag = "10")]
    pub pending: Option<StoredPendingPreKey>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredSession {
    #[prost(message, optional, tag = "1")]
    pub current: Option<StoredSessionState>,
    #[prost(message, repeated, tag = "2")]
    pub previous: Vec<StoredSessionState>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredSenderKeyState {
    #[prost(uint32, tag = "1")]
    pub key_id: u32,
    #[prost(uint32, tag = "2")]
    pub iteration: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub chain_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub signing_public: Vec<u8>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub signing_private: Option<Vec<u8>>,
    #[prost(message, repeated, tag = "6")]
    pub message_keys: Vec<StoredMessageKey>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StoredSenderKey {
    #[prost(message, repeated, tag = "1")]
    pub states: Vec<StoredSenderKeyState>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalRecordStructure {
    #[prost(message, optional, tag = "1")]
    pub current_session: Option<SignalSessionStructure>,
    #[prost(message, repeated, tag = "2")]
    pub previous_sessions: Vec<SignalSessionStructure>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalSessionStructure {
    #[prost(uint32, optional, tag = "1")]
    pub session_version: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub local_identity_public: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub remote_identity_public: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub root_key: Option<Vec<u8>>,
    #[prost(uint32, optional, tag = "5")]
    pub previous_counter: Option<u32>,
    #[prost(message, optional, tag = "6")]
    pub sender_chain: Option<SignalChain>,
    #[prost(message, repeated, tag = "7")]
    pub receiver_chains: Vec<SignalChain>,
    #[prost(message, optional, tag = "9")]
    pub pending_pre_key: Option<SignalPendingPreKey>,
    #[prost(uint32, optional, tag = "10")]
    pub remote_registration_id: Option<u32>,
    #[prost(uint32, optional, tag = "11")]
    pub local_registration_id: Option<u32>,
    #[prost(bool, optional, tag = "12")]
    pub needs_refresh: Option<bool>,
    #[prost(bytes = "vec", optional, tag = "13")]
    pub alice_base_key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalChain {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub sender_ratchet_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub sender_ratchet_key_private: Option<Vec<u8>>,
    #[prost(message, optional, tag = "3")]
    pub chain_key: Option<SignalChainKey>,
    #[prost(message, repeated, tag = "4")]
    pub message_keys: Vec<SignalMessageKeys>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalChainKey {
    #[prost(uint32, optional, tag = "1")]
    pub index: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalMessageKeys {
    #[prost(uint32, optional, tag = "1")]
    pub index: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub cipher_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub mac_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub iv: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalPendingPreKey {
    #[prost(uint32, optional, tag = "1")]
    pub pre_key_id: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub base_key: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "3")]
    pub signed_pre_key_id: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderKeyRecordStructure {
    #[prost(message, repeated, tag = "1")]
    pub sender_key_states: Vec<SenderKeyStateStructure>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderKeyStateStructure {
    #[prost(uint32, optional, tag = "1")]
    pub sender_key_id: Option<u32>,
    #[prost(message, optional, tag = "2")]
    pub sender_chain_key: Option<SenderChainKeyStructure>,
    #[prost(message, optional, tag = "3")]
    pub sender_signing_key: Option<SenderSigningKeyStructure>,
    #[prost(message, repeated, tag = "4")]
    pub sender_message_keys: Vec<SenderMessageKeyStructure>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderChainKeyStructure {
    #[prost(uint32, optional, tag = "1")]
    pub iteration: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub seed: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderSigningKeyStructure {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub public: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub private: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SenderMessageKeyStructure {
    #[prost(uint32, optional, tag = "1")]
    pub iteration: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub seed: Option<Vec<u8>>,
}
