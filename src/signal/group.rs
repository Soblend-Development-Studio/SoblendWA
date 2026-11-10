use prost::Message as _;

use super::session::VERSION_BYTE;
use crate::crypto::{self, signal_key, strip_key_prefix, verify, KeyPair};
use crate::error::{Error, Result};
use crate::proto::{SenderKeyDistribution, SenderKeyMessage, StoredMessageKey, StoredSenderKey, StoredSenderKeyState};

const MAX_STATES: usize = 5;
const MAX_SKIPPED_KEYS: usize = 2000;
const MAX_FORWARD_JUMP: u32 = 2000;
const SIGNATURE_LEN: usize = 64;

#[derive(Clone)]
struct SenderKeyState {
    key_id: u32,
    iteration: u32,
    chain_key: [u8; 32],
    signing_public: [u8; 32],
    signing_private: Option<[u8; 32]>,
    skipped: Vec<(u32, [u8; 32])>,
    edwards: Option<[u8; 32]>,
}

impl SenderKeyState {
    fn advance(&mut self) -> (u32, [u8; 32]) {
        let seed = crypto::hmac_sha256(&self.chain_key, &[&[1]]);
        let used = self.iteration;
        self.chain_key = crypto::hmac_sha256(&self.chain_key, &[&[2]]);
        self.iteration += 1;
        (used, seed)
    }

    fn key_for(&mut self, iteration: u32) -> Result<[u8; 32]> {
        if iteration < self.iteration {
            let index = self
                .skipped
                .iter()
                .position(|(i, _)| *i == iteration)
                .ok_or_else(|| Error::signal("sender key for this iteration was already used"))?;
            return Ok(self.skipped.remove(index).1);
        }
        if iteration - self.iteration > MAX_FORWARD_JUMP {
            return Err(Error::signal("group message is too far into the future"));
        }
        while self.iteration < iteration {
            let skipped = self.advance();
            self.skipped.push(skipped);
        }
        if self.skipped.len() > MAX_SKIPPED_KEYS {
            let excess = self.skipped.len() - MAX_SKIPPED_KEYS;
            self.skipped.drain(..excess);
        }
        Ok(self.advance().1)
    }
}

fn message_keys(seed: &[u8; 32]) -> ([u8; 16], [u8; 32]) {
    let derived: [u8; 48] = crypto::hkdf_array(seed, None, b"WhisperGroup");
    let mut iv = [0u8; 16];
    let mut key = [0u8; 32];
    iv.copy_from_slice(&derived[..16]);
    key.copy_from_slice(&derived[16..]);
    (iv, key)
}

#[derive(Clone, Default)]
pub(crate) struct SenderKeyRecord {
    states: Vec<SenderKeyState>,
}

impl SenderKeyRecord {
    pub(crate) fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub(crate) fn create_own() -> Self {
        let signing = KeyPair::generate();
        SenderKeyRecord {
            states: vec![SenderKeyState {
                key_id: crypto::random_u32() & 0x7fff_ffff,
                iteration: 0,
                chain_key: crypto::random_array(),
                signing_public: signing.public,
                signing_private: Some(signing.private),
                edwards: None,
                skipped: Vec::new(),
            }],
        }
    }

    pub(crate) fn distribution_message(&self) -> Result<Vec<u8>> {
        let state = self.states.first().ok_or_else(|| Error::signal("no sender key state"))?;
        let body = SenderKeyDistribution {
            id: Some(state.key_id),
            iteration: Some(state.iteration),
            chain_key: Some(state.chain_key.to_vec()),
            signing_key: Some(signal_key(&state.signing_public).to_vec()),
        };
        let mut out = Vec::with_capacity(1 + body.encoded_len());
        out.push(VERSION_BYTE);
        body.encode(&mut out).expect("vec has capacity");
        Ok(out)
    }

    pub(crate) fn process_distribution(&mut self, data: &[u8]) -> Result<()> {
        if data.len() < 2 || data[0] >> 4 != 3 {
            return Err(Error::signal("malformed sender key distribution message"));
        }
        let body = SenderKeyDistribution::decode(&data[1..])?;
        let key_id = body.id.unwrap_or(0);
        let state = SenderKeyState {
            key_id,
            iteration: body.iteration.unwrap_or(0),
            chain_key: body
                .chain_key
                .as_deref()
                .and_then(|k| k.try_into().ok())
                .ok_or_else(|| Error::signal("invalid sender chain key"))?,
            signing_public: strip_key_prefix(body.signing_key.as_deref().unwrap_or_default())?,
            signing_private: None,
            edwards: None,
            skipped: Vec::new(),
        };
        if let Some(existing) = self.states.iter().find(|s| s.key_id == key_id) {
            if existing.signing_public == state.signing_public && existing.iteration > state.iteration {
                return Ok(());
            }
        }
        self.states.retain(|s| s.key_id != key_id);
        self.states.insert(0, state);
        self.states.truncate(MAX_STATES);
        Ok(())
    }

    pub(crate) fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let state = self.states.first_mut().ok_or_else(|| Error::signal("no sender key state"))?;
        let signing_private = state.signing_private.ok_or_else(|| Error::signal("sender key is not ours"))?;
        let (iteration, seed) = state.advance();
        let (iv, key) = message_keys(&seed);
        let body = SenderKeyMessage {
            id: Some(state.key_id),
            iteration: Some(iteration),
            ciphertext: Some(crypto::aes_cbc_encrypt(&key, &iv, plaintext)?),
        };
        let mut out = Vec::with_capacity(1 + body.encoded_len() + SIGNATURE_LEN);
        out.push(VERSION_BYTE);
        body.encode(&mut out).expect("vec has capacity");
        let edwards = *state.edwards.get_or_insert_with(|| crypto::edwards_public(&signing_private));
        let signature = crypto::sign_with(&signing_private, &edwards, &out);
        out.extend_from_slice(&signature);
        Ok(out)
    }

    pub(crate) fn decrypt(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 1 + SIGNATURE_LEN || data[0] >> 4 != 3 {
            return Err(Error::signal("malformed sender key message"));
        }
        let (signed, signature) = data.split_at(data.len() - SIGNATURE_LEN);
        let body = SenderKeyMessage::decode(&signed[1..])?;
        let key_id = body.id.unwrap_or(0);
        let state = self
            .states
            .iter_mut()
            .find(|s| s.key_id == key_id)
            .ok_or_else(|| Error::signal("no sender key state for this key id"))?;
        let signature: &[u8; 64] = signature.try_into().expect("split at signature length");
        if !verify(&state.signing_public, signed, signature) {
            return Err(Error::signal("sender key signature is invalid"));
        }
        let mut attempt = state.clone();
        let seed = attempt.key_for(body.iteration.unwrap_or(0))?;
        let (iv, key) = message_keys(&seed);
        let plaintext = crypto::aes_cbc_decrypt(&key, &iv, body.ciphertext.as_deref().unwrap_or_default())?;
        *state = attempt;
        Ok(plaintext)
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        StoredSenderKey {
            states: self
                .states
                .iter()
                .map(|s| StoredSenderKeyState {
                    key_id: s.key_id,
                    iteration: s.iteration,
                    chain_key: s.chain_key.to_vec(),
                    signing_public: s.signing_public.to_vec(),
                    signing_private: s.signing_private.map(|k| k.to_vec()),
                    message_keys: s
                        .skipped
                        .iter()
                        .map(|(index, key)| StoredMessageKey { index: *index, key: key.to_vec() })
                        .collect(),
                })
                .collect(),
        }
        .encode_to_vec()
    }

    #[cfg(any(feature = "sqlite", feature = "mongo"))]
    pub(crate) fn from_standard(bytes: &[u8]) -> Result<Self> {
        use crate::proto::SenderKeyRecordStructure;
        let fixed = |raw: Option<&[u8]>, what: &str| -> Result<[u8; 32]> {
            let raw = raw.ok_or_else(|| Error::signal(format!("sender key has no {what}")))?;
            match raw.len() {
                33 => strip_key_prefix(raw),
                _ => raw.try_into().map_err(|_| Error::signal(format!("sender key {what} has the wrong length"))),
            }
        };
        let record = SenderKeyRecordStructure::decode(bytes)?;
        let mut states = Vec::with_capacity(record.sender_key_states.len());
        for state in &record.sender_key_states {
            let chain = state.sender_chain_key.as_ref();
            let signing = state.sender_signing_key.as_ref();
            states.push(SenderKeyState {
                key_id: state.sender_key_id.unwrap_or(0),
                iteration: chain.and_then(|c| c.iteration).unwrap_or(0),
                chain_key: fixed(chain.and_then(|c| c.seed.as_deref()), "chain seed")?,
                signing_public: fixed(signing.and_then(|s| s.public.as_deref()), "signing key")?,
                signing_private: signing
                    .and_then(|s| s.private.as_deref())
                    .filter(|p| !p.is_empty())
                    .map(|p| fixed(Some(p), "signing private key"))
                    .transpose()?,
                edwards: None,
                skipped: state
                    .sender_message_keys
                    .iter()
                    .filter_map(|k| Some((k.iteration?, fixed(k.seed.as_deref(), "message seed").ok()?)))
                    .collect(),
            });
        }
        states.truncate(MAX_STATES);
        Ok(SenderKeyRecord { states })
    }

    #[cfg(any(feature = "sqlite", feature = "mongo"))]
    pub(crate) fn to_standard(&self) -> Vec<u8> {
        use crate::proto::{
            SenderChainKeyStructure, SenderKeyRecordStructure, SenderKeyStateStructure, SenderMessageKeyStructure,
            SenderSigningKeyStructure,
        };
        SenderKeyRecordStructure {
            sender_key_states: self
                .states
                .iter()
                .map(|s| SenderKeyStateStructure {
                    sender_key_id: Some(s.key_id),
                    sender_chain_key: Some(SenderChainKeyStructure {
                        iteration: Some(s.iteration),
                        seed: Some(s.chain_key.to_vec()),
                    }),
                    sender_signing_key: Some(SenderSigningKeyStructure {
                        public: Some(signal_key(&s.signing_public).to_vec()),
                        private: s.signing_private.map(|p| p.to_vec()),
                    }),
                    sender_message_keys: s
                        .skipped
                        .iter()
                        .map(|(iteration, seed)| SenderMessageKeyStructure {
                            iteration: Some(*iteration),
                            seed: Some(seed.to_vec()),
                        })
                        .collect(),
                })
                .collect(),
        }
        .encode_to_vec()
    }

    pub(crate) fn from_baileys(value: &serde_json::Value) -> Result<Self> {
        use crate::signal::json_bytes;
        let parsed;
        let states = match value {
            serde_json::Value::Array(states) => states,
            other => {
                let raw = json_bytes(other).ok_or_else(|| Error::signal("baileys sender key is not readable"))?;
                parsed = serde_json::from_slice::<serde_json::Value>(&raw)?;
                parsed.as_array().ok_or_else(|| Error::signal("baileys sender key is not a list of states"))?
            }
        };
        let key = |value: &serde_json::Value| -> Result<[u8; 32]> {
            let raw = json_bytes(value).ok_or_else(|| Error::signal("baileys sender key field is missing"))?;
            match raw.len() {
                33 => strip_key_prefix(&raw),
                _ => raw.as_slice().try_into().map_err(|_| Error::signal("baileys sender key has the wrong length")),
            }
        };
        let number = |value: &serde_json::Value| value.as_u64().or_else(|| value.as_str()?.parse().ok());
        let mut imported = Vec::with_capacity(states.len());
        for state in states.iter().rev() {
            let chain = &state["senderChainKey"];
            let signing = &state["senderSigningKey"];
            let skipped = state["senderMessageKeys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| Some((number(&k["iteration"])? as u32, key(&k["seed"]).ok()?)))
                .collect();
            imported.push(SenderKeyState {
                key_id: number(&state["senderKeyId"]).unwrap_or(0) as u32,
                iteration: number(&chain["iteration"]).unwrap_or(0) as u32,
                chain_key: key(&chain["seed"])?,
                signing_public: key(&signing["public"])?,
                signing_private: signing.get("private").filter(|p| !p.is_null()).map(key).transpose()?,
                edwards: None,
                skipped,
            });
        }
        imported.truncate(MAX_STATES);
        Ok(SenderKeyRecord { states: imported })
    }

    pub(crate) fn from_bytes(data: &[u8]) -> Result<Self> {
        let fixed =
            |v: &[u8]| -> Result<[u8; 32]> { v.try_into().map_err(|_| Error::signal("stored sender key is corrupt")) };
        let stored = StoredSenderKey::decode(data)?;
        let states = stored
            .states
            .into_iter()
            .map(|s| {
                Ok(SenderKeyState {
                    key_id: s.key_id,
                    iteration: s.iteration,
                    chain_key: fixed(&s.chain_key)?,
                    signing_public: fixed(&s.signing_public)?,
                    signing_private: s.signing_private.as_deref().map(fixed).transpose()?,
                    edwards: None,
                    skipped: s.message_keys.iter().map(|k| Ok((k.index, fixed(&k.key)?))).collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(SenderKeyRecord { states })
    }
}
