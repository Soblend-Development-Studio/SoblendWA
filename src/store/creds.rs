use serde::{Deserialize, Serialize};

use crate::crypto::{self, KeyPair};
use crate::jid::Jid;
use crate::serde_b64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedPreKey {
    pub id: u32,
    pub key_pair: KeyPair,
    #[serde(with = "serde_b64::array")]
    pub signature: [u8; 64],
}

impl SignedPreKey {
    pub fn generate(identity: &KeyPair, id: u32) -> Self {
        let key_pair = KeyPair::generate();
        let signature = identity.sign(&key_pair.signal_public());
        SignedPreKey { id, key_pair, signature }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Creds {
    pub noise_key: KeyPair,
    pub pairing_ephemeral_key: KeyPair,
    pub identity_key: KeyPair,
    pub signed_pre_key: SignedPreKey,
    pub registration_id: u32,
    #[serde(with = "serde_b64::array")]
    pub adv_secret_key: [u8; 32],
    pub next_pre_key_id: u32,
    pub first_unuploaded_pre_key_id: u32,
    #[serde(default)]
    pub me: Option<Jid>,
    #[serde(default)]
    pub lid: Option<Jid>,
    #[serde(default)]
    pub push_name: Option<String>,
    #[serde(default, with = "serde_b64::option")]
    pub account: Option<Vec<u8>>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub business_name: Option<String>,
    #[serde(default)]
    pub pairing_code: Option<String>,
    #[serde(default, with = "serde_b64::option")]
    pub routing_info: Option<Vec<u8>>,
    #[serde(default)]
    pub registered: bool,
    #[serde(default)]
    pub wa_version: Option<[u32; 3]>,
    #[serde(default)]
    pub app_state_key_id: Option<String>,
    #[serde(default)]
    pub signed_pre_key_rotated_at: i64,
}

impl Creds {
    pub fn generate() -> Self {
        let identity_key = KeyPair::generate();
        let signed_pre_key = SignedPreKey::generate(&identity_key, 1);
        Creds {
            noise_key: KeyPair::generate(),
            pairing_ephemeral_key: KeyPair::generate(),
            identity_key,
            signed_pre_key,
            registration_id: (crypto::random_u32() & 0x3fff).max(1),
            adv_secret_key: crypto::random_array(),
            next_pre_key_id: 1,
            first_unuploaded_pre_key_id: 1,
            me: None,
            lid: None,
            push_name: None,
            account: None,
            platform: None,
            business_name: None,
            pairing_code: None,
            routing_info: None,
            registered: false,
            wa_version: None,
            app_state_key_id: None,
            signed_pre_key_rotated_at: 0,
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.me.is_some() && self.account.is_some()
    }
}

impl Default for Creds {
    fn default() -> Self {
        Self::generate()
    }
}
