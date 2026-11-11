use prost::Message as _;

use crate::crypto::{self, signal_key, strip_key_prefix, verify, KeyPair};
use crate::error::{Error, Result};
use crate::proto::{
    PreKeyWhisperMessage, StoredChain, StoredDerivedKey, StoredMessageKey, StoredPendingPreKey, StoredSession,
    StoredSessionState, WhisperMessage,
};

pub(crate) const VERSION_BYTE: u8 = 0x33;
const MAC_LEN: usize = 8;
// Same limits as libsignal, so a session that works there works here and the other way around.
const MAX_PREVIOUS_SESSIONS: usize = 40;
const MAX_RECEIVING_CHAINS: usize = 5;
const MAX_SKIPPED_KEYS: usize = 2000;
const MAX_FORWARD_JUMP: i64 = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncType {
    PreKey,
    Message,
}

impl EncType {
    pub fn as_str(self) -> &'static str {
        match self {
            EncType::PreKey => "pkmsg",
            EncType::Message => "msg",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreKeyBundle {
    pub registration_id: u32,
    pub identity_key: [u8; 32],
    pub signed_pre_key_id: u32,
    pub signed_pre_key: [u8; 32],
    pub signed_pre_key_signature: [u8; 64],
    pub pre_key: Option<(u32, [u8; 32])>,
}

pub(crate) fn derive_secrets<const N: usize>(input: &[u8], salt: &[u8; 32], info: &[u8]) -> [[u8; 32]; N] {
    let okm = crypto::hkdf(input, Some(salt), info, 32 * N);
    let mut out = [[0u8; 32]; N];
    for (slot, chunk) in out.iter_mut().zip(okm.as_chunks::<32>().0) {
        *slot = *chunk;
    }
    out
}

#[derive(Clone, Copy)]
struct MessageSecrets {
    cipher: [u8; 32],
    mac: [u8; 32],
    iv: [u8; 16],
}

impl MessageSecrets {
    fn from_seed(seed: &[u8; 32]) -> Self {
        let [cipher, mac, iv] = derive_secrets::<3>(seed, &[0; 32], b"WhisperMessageKeys");
        let mut short = [0u8; 16];
        short.copy_from_slice(&iv[..16]);
        MessageSecrets { cipher, mac, iv: short }
    }
}

#[derive(Clone)]
struct Chain {
    ratchet: [u8; 32],
    key: Option<[u8; 32]>,
    counter: i64,
    skipped: Vec<(u32, [u8; 32])>,
    derived: Vec<(u32, MessageSecrets)>,
}

impl Chain {
    fn new(ratchet: [u8; 32], key: [u8; 32]) -> Self {
        Chain { ratchet, key: Some(key), counter: -1, skipped: Vec::new(), derived: Vec::new() }
    }

    fn next_key(&mut self) -> Result<(u32, [u8; 32])> {
        let key = self.key.ok_or_else(|| Error::signal("sending chain is closed"))?;
        self.key = Some(crypto::hmac_sha256(&key, &[&[2]]));
        self.counter += 1;
        Ok((self.counter as u32, crypto::hmac_sha256(&key, &[&[1]])))
    }

    fn advance_to(&mut self, target: i64) -> Result<()> {
        if self.counter >= target {
            return Ok(());
        }
        if target - self.counter > MAX_FORWARD_JUMP {
            return Err(Error::signal("message is too far into the future"));
        }
        let mut key = self.key.ok_or_else(|| Error::signal("receiving chain is closed"))?;
        while self.counter < target {
            self.counter += 1;
            self.skipped.push((self.counter as u32, crypto::hmac_sha256(&key, &[&[1]])));
            key = crypto::hmac_sha256(&key, &[&[2]]);
        }
        self.key = Some(key);
        if self.skipped.len() > MAX_SKIPPED_KEYS {
            let excess = self.skipped.len() - MAX_SKIPPED_KEYS;
            self.skipped.drain(..excess);
        }
        Ok(())
    }

    fn take_secrets(&mut self, counter: u32) -> Option<MessageSecrets> {
        if let Some(index) = self.skipped.iter().position(|(c, _)| *c == counter) {
            return Some(MessageSecrets::from_seed(&self.skipped.remove(index).1));
        }
        let index = self.derived.iter().position(|(c, _)| *c == counter)?;
        Some(self.derived.remove(index).1)
    }
}

#[derive(Clone)]
struct PendingPreKey {
    pre_key_id: Option<u32>,
    signed_pre_key_id: u32,
    base_key: [u8; 32],
}

#[derive(Clone)]
pub(crate) struct SessionState {
    registration_id: u32,
    root_key: [u8; 32],
    our_ratchet: KeyPair,
    their_ratchet: [u8; 32],
    previous_counter: u32,
    sending: Option<Chain>,
    receiving: Vec<Chain>,
    remote_identity: [u8; 32],
    base_key: [u8; 32],
    pending: Option<PendingPreKey>,
}

impl SessionState {
    pub(crate) fn initiate(our_identity: &KeyPair, bundle: &PreKeyBundle) -> Result<Self> {
        let signed = signal_key(&bundle.signed_pre_key);
        if !verify(&bundle.identity_key, &signed, &bundle.signed_pre_key_signature) {
            return Err(Error::signal("signed pre-key signature is invalid"));
        }
        let base = KeyPair::generate();
        let mut secret = Vec::with_capacity(160);
        secret.extend_from_slice(&[0xff; 32]);
        secret.extend_from_slice(&our_identity.shared_secret(&bundle.signed_pre_key));
        secret.extend_from_slice(&base.shared_secret(&bundle.identity_key));
        secret.extend_from_slice(&base.shared_secret(&bundle.signed_pre_key));
        if let Some((_, one_time)) = &bundle.pre_key {
            secret.extend_from_slice(&base.shared_secret(one_time));
        }
        let [root_key, _] = derive_secrets::<2>(&secret, &[0; 32], b"WhisperText");

        let mut state = SessionState {
            registration_id: bundle.registration_id,
            root_key,
            our_ratchet: KeyPair::generate(),
            their_ratchet: bundle.signed_pre_key,
            previous_counter: 0,
            sending: None,
            receiving: Vec::new(),
            remote_identity: bundle.identity_key,
            base_key: base.public,
            pending: Some(PendingPreKey {
                pre_key_id: bundle.pre_key.map(|(id, _)| id),
                signed_pre_key_id: bundle.signed_pre_key_id,
                base_key: base.public,
            }),
        };
        let chain_key = state.ratchet_root(&bundle.signed_pre_key);
        state.sending = Some(Chain::new(state.our_ratchet.public, chain_key));
        Ok(state)
    }

    fn respond(
        our_identity: &KeyPair,
        signed_pre_key: &KeyPair,
        one_time: Option<&KeyPair>,
        their_identity: [u8; 32],
        their_base: [u8; 32],
        registration_id: u32,
    ) -> Self {
        let mut secret = Vec::with_capacity(160);
        secret.extend_from_slice(&[0xff; 32]);
        secret.extend_from_slice(&signed_pre_key.shared_secret(&their_identity));
        secret.extend_from_slice(&our_identity.shared_secret(&their_base));
        secret.extend_from_slice(&signed_pre_key.shared_secret(&their_base));
        if let Some(one_time) = one_time {
            secret.extend_from_slice(&one_time.shared_secret(&their_base));
        }
        let [root_key, _] = derive_secrets::<2>(&secret, &[0; 32], b"WhisperText");
        SessionState {
            registration_id,
            root_key,
            our_ratchet: signed_pre_key.clone(),
            their_ratchet: their_base,
            previous_counter: 0,
            sending: None,
            receiving: Vec::new(),
            remote_identity: their_identity,
            base_key: their_base,
            pending: None,
        }
    }

    fn ratchet_root(&mut self, their_ratchet: &[u8; 32]) -> [u8; 32] {
        let shared = self.our_ratchet.shared_secret(their_ratchet);
        let [root_key, chain_key] = derive_secrets::<2>(&shared, &self.root_key, b"WhisperRatchet");
        self.root_key = root_key;
        chain_key
    }

    fn step(&mut self, their_ratchet: [u8; 32], previous_counter: u32) -> Result<()> {
        if self.receiving.iter().any(|c| c.ratchet == their_ratchet) {
            return Ok(());
        }
        let last = self.their_ratchet;
        if let Some(previous) = self.receiving.iter_mut().find(|c| c.ratchet == last) {
            previous.advance_to(i64::from(previous_counter))?;
            previous.key = None;
        }

        let receiving_key = self.ratchet_root(&their_ratchet);
        self.receiving.push(Chain::new(their_ratchet, receiving_key));
        if self.receiving.len() > MAX_RECEIVING_CHAINS {
            self.receiving.remove(0);
        }

        if let Some(sending) = self.sending.take() {
            self.previous_counter = sending.counter.max(0) as u32;
        }
        self.our_ratchet = KeyPair::generate();
        let sending_key = self.ratchet_root(&their_ratchet);
        self.sending = Some(Chain::new(self.our_ratchet.public, sending_key));
        self.their_ratchet = their_ratchet;
        Ok(())
    }

    fn encrypt(
        &mut self,
        our_identity: &KeyPair,
        our_registration_id: u32,
        plaintext: &[u8],
    ) -> Result<(EncType, Vec<u8>)> {
        let sending = self.sending.as_mut().ok_or_else(|| Error::signal("session has no sending chain"))?;
        let (counter, message_key) = sending.next_key()?;
        let [cipher_key, mac_key, iv] = derive_secrets::<3>(&message_key, &[0; 32], b"WhisperMessageKeys");
        let ciphertext = crypto::aes_cbc_encrypt(&cipher_key, &iv[..16], plaintext)?;

        let body = WhisperMessage {
            ephemeral_key: Some(signal_key(&self.our_ratchet.public).to_vec()),
            counter: Some(counter),
            previous_counter: Some(self.previous_counter),
            ciphertext: Some(ciphertext),
        }
        .encode_to_vec();
        let mac = crypto::hmac_sha256(
            &mac_key,
            &[&our_identity.signal_public(), &signal_key(&self.remote_identity), &[VERSION_BYTE], &body],
        );
        let mut whisper = Vec::with_capacity(1 + body.len() + MAC_LEN);
        whisper.push(VERSION_BYTE);
        whisper.extend_from_slice(&body);
        whisper.extend_from_slice(&mac[..MAC_LEN]);

        let Some(pending) = &self.pending else {
            return Ok((EncType::Message, whisper));
        };
        let envelope = PreKeyWhisperMessage {
            pre_key_id: pending.pre_key_id,
            base_key: Some(signal_key(&pending.base_key).to_vec()),
            identity_key: Some(our_identity.signal_public().to_vec()),
            message: Some(whisper),
            registration_id: Some(our_registration_id),
            signed_pre_key_id: Some(pending.signed_pre_key_id),
        };
        let mut out = Vec::with_capacity(1 + envelope.encoded_len());
        out.push(VERSION_BYTE);
        envelope.encode(&mut out).expect("vec has capacity");
        Ok((EncType::PreKey, out))
    }

    fn decrypt(&mut self, our_identity: &KeyPair, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() <= 1 + MAC_LEN || data[0] >> 4 != 3 {
            return Err(Error::signal("malformed whisper message"));
        }
        let (body, mac) = data[1..].split_at(data.len() - 1 - MAC_LEN);
        let message = WhisperMessage::decode(body)?;
        let their_ratchet = strip_key_prefix(message.ephemeral_key.as_deref().unwrap_or_default())?;
        let counter = message.counter.unwrap_or(0);

        self.step(their_ratchet, message.previous_counter.unwrap_or(0))?;
        let chain = self
            .receiving
            .iter_mut()
            .find(|c| c.ratchet == their_ratchet)
            .ok_or_else(|| Error::signal("missing receiving chain"))?;
        chain.advance_to(i64::from(counter))?;
        let secrets = chain.take_secrets(counter).ok_or_else(|| Error::signal("message key was already used"))?;

        let expected = crypto::hmac_sha256(
            &secrets.mac,
            &[&signal_key(&self.remote_identity), &our_identity.signal_public(), &[VERSION_BYTE], body],
        );
        if !crypto::constant_time_eq(&expected[..MAC_LEN], mac) {
            return Err(Error::signal("bad message mac"));
        }
        let plaintext =
            crypto::aes_cbc_decrypt(&secrets.cipher, &secrets.iv, message.ciphertext.as_deref().unwrap_or_default())?;
        self.pending = None;
        Ok(plaintext)
    }
}

pub(crate) struct IncomingPreKey {
    pub base_key: [u8; 32],
    pub identity_key: [u8; 32],
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: Option<u32>,
    pub registration_id: u32,
    pub message: Vec<u8>,
}

impl IncomingPreKey {
    pub(crate) fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 2 || data[0] >> 4 != 3 {
            return Err(Error::signal("malformed pre-key message"));
        }
        let envelope = PreKeyWhisperMessage::decode(&data[1..])?;
        Ok(IncomingPreKey {
            base_key: strip_key_prefix(envelope.base_key.as_deref().unwrap_or_default())?,
            identity_key: strip_key_prefix(envelope.identity_key.as_deref().unwrap_or_default())?,
            pre_key_id: envelope.pre_key_id,
            signed_pre_key_id: envelope.signed_pre_key_id,
            registration_id: envelope.registration_id.unwrap_or(0),
            message: envelope.message.ok_or_else(|| Error::signal("pre-key message without payload"))?,
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct SessionRecord {
    current: Option<SessionState>,
    previous: Vec<SessionState>,
}

impl SessionRecord {
    #[cfg(feature = "testing")]
    pub(crate) fn scramble(&mut self) {
        for state in self.current.iter_mut().chain(self.previous.iter_mut()) {
            state.root_key[0] ^= 0xff;
            for chain in &mut state.receiving {
                chain.ratchet[0] ^= 0xff;
                if let Some(key) = chain.key.as_mut() {
                    key[0] ^= 0xff;
                }
            }
        }
    }

    pub(crate) fn has_session(&self) -> bool {
        self.current.is_some()
    }

    pub(crate) fn has_base_key(&self, base_key: &[u8; 32]) -> bool {
        self.current.iter().chain(&self.previous).any(|s| s.base_key == *base_key)
    }

    pub(crate) fn promote(&mut self, state: SessionState) {
        if let Some(old) = self.current.take() {
            self.previous.insert(0, old);
            self.previous.truncate(MAX_PREVIOUS_SESSIONS);
        }
        self.current = Some(state);
    }

    pub(crate) fn encrypt(
        &mut self,
        our_identity: &KeyPair,
        our_registration_id: u32,
        plaintext: &[u8],
    ) -> Result<(EncType, Vec<u8>)> {
        self.current.as_mut().ok_or_else(|| Error::signal("no open session"))?.encrypt(
            our_identity,
            our_registration_id,
            plaintext,
        )
    }

    pub(crate) fn decrypt(&mut self, our_identity: &KeyPair, data: &[u8]) -> Result<Vec<u8>> {
        let mut last_error = Error::signal("no sessions available");
        if let Some(current) = &self.current {
            let mut attempt = current.clone();
            match attempt.decrypt(our_identity, data) {
                Ok(plaintext) => {
                    self.current = Some(attempt);
                    return Ok(plaintext);
                }
                Err(err) => last_error = err,
            }
        }
        for index in 0..self.previous.len() {
            let mut attempt = self.previous[index].clone();
            if let Ok(plaintext) = attempt.decrypt(our_identity, data) {
                self.previous.remove(index);
                self.promote(attempt);
                return Ok(plaintext);
            }
        }
        Err(last_error)
    }

    pub(crate) fn decrypt_pre_key(
        &mut self,
        our_identity: &KeyPair,
        signed_pre_key: &KeyPair,
        one_time: Option<&KeyPair>,
        incoming: &IncomingPreKey,
    ) -> Result<Vec<u8>> {
        if let Some(current) = self.current.as_ref().filter(|s| s.base_key == incoming.base_key) {
            let mut attempt = current.clone();
            let plaintext = attempt.decrypt(our_identity, &incoming.message)?;
            self.current = Some(attempt);
            return Ok(plaintext);
        }
        if let Some(index) = self.previous.iter().position(|s| s.base_key == incoming.base_key) {
            let mut attempt = self.previous[index].clone();
            let plaintext = attempt.decrypt(our_identity, &incoming.message)?;
            self.previous.remove(index);
            self.promote(attempt);
            return Ok(plaintext);
        }

        let mut state = SessionState::respond(
            our_identity,
            signed_pre_key,
            one_time,
            incoming.identity_key,
            incoming.base_key,
            incoming.registration_id,
        );
        let plaintext = state.decrypt(our_identity, &incoming.message)?;
        self.promote(state);
        Ok(plaintext)
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        StoredSession {
            current: self.current.as_ref().map(state_to_proto),
            previous: self.previous.iter().map(state_to_proto).collect(),
        }
        .encode_to_vec()
    }

    pub(crate) fn from_bytes(data: &[u8]) -> Result<Self> {
        let stored = StoredSession::decode(data)?;
        Ok(SessionRecord {
            current: stored.current.map(state_from_proto).transpose()?,
            previous: stored.previous.into_iter().map(state_from_proto).collect::<Result<_>>()?,
        })
    }
}

fn array32(data: &[u8]) -> Result<[u8; 32]> {
    data.try_into().map_err(|_| Error::signal("stored key has the wrong length"))
}

mod libsignal {
    use serde_json::Value;

    use super::*;
    use crate::signal::json_bytes;

    fn key(value: &Value, what: &str) -> Result<[u8; 32]> {
        let raw = json_bytes(value).ok_or_else(|| Error::signal(format!("libsignal session has no {what}")))?;
        match raw.len() {
            33 => strip_key_prefix(&raw),
            _ => array32(&raw),
        }
    }

    fn number(value: &Value) -> Option<i64> {
        value.as_i64().or_else(|| value.as_str().and_then(|s| s.parse().ok()))
    }

    fn chain(ratchet_b64: &str, value: &Value) -> Result<(i64, Chain)> {
        let ratchet = key(&Value::String(ratchet_b64.to_owned()), "chain ratchet key")?;
        let chain_key = &value["chainKey"];
        let skipped = value["messageKeys"]
            .as_object()
            .map(|keys| {
                let mut skipped = keys
                    .iter()
                    .filter_map(|(index, seed)| Some((index.parse::<u32>().ok()?, key(seed, "message key").ok()?)))
                    .collect::<Vec<_>>();
                skipped.sort_by_key(|(index, _)| *index);
                skipped
            })
            .unwrap_or_default();
        let parsed = Chain {
            ratchet,
            key: chain_key.get("key").filter(|k| !k.is_null()).map(|k| key(k, "chain key")).transpose()?,
            counter: number(&chain_key["counter"]).unwrap_or(-1),
            skipped,
            derived: Vec::new(),
        };
        Ok((number(&value["chainType"]).unwrap_or(0), parsed))
    }

    fn state(entry: &Value) -> Result<SessionState> {
        let ratchet = &entry["currentRatchet"];
        let index = &entry["indexInfo"];
        let ours = KeyPair::from_private(key(&ratchet["ephemeralKeyPair"]["privKey"], "ratchet private key")?);
        if ours.public != key(&ratchet["ephemeralKeyPair"]["pubKey"], "ratchet public key")? {
            return Err(Error::signal("libsignal session has an inconsistent ratchet key pair"));
        }
        let their_ratchet = key(&ratchet["lastRemoteEphemeralKey"], "remote ratchet key")?;
        let mut sending = None;
        let mut receiving = Vec::new();
        for (ratchet_b64, value) in entry["_chains"].as_object().into_iter().flatten() {
            match chain(ratchet_b64, value)? {
                (1, chain) if chain.ratchet == ours.public => sending = Some(chain),
                (2, chain) => receiving.push(chain),
                _ => {}
            }
        }
        receiving.sort_by_key(|c| c.ratchet == their_ratchet);
        if receiving.len() > MAX_RECEIVING_CHAINS {
            receiving.drain(..receiving.len() - MAX_RECEIVING_CHAINS);
        }
        let pending = entry.get("pendingPreKey").filter(|p| p.is_object()).map(|p| {
            Ok::<_, Error>(PendingPreKey {
                pre_key_id: number(&p["preKeyId"]).map(|id| id as u32),
                signed_pre_key_id: number(&p["signedKeyId"]).unwrap_or(0) as u32,
                base_key: key(&p["baseKey"], "pending base key")?,
            })
        });
        Ok(SessionState {
            registration_id: number(&entry["registrationId"]).unwrap_or(0) as u32,
            root_key: key(&ratchet["rootKey"], "root key")?,
            our_ratchet: ours,
            their_ratchet,
            previous_counter: number(&ratchet["previousCounter"]).unwrap_or(0).max(0) as u32,
            sending,
            receiving,
            remote_identity: key(&index["remoteIdentityKey"], "remote identity key")?,
            base_key: key(&index["baseKey"], "base key")?,
            pending: pending.transpose()?,
        })
    }

    pub(crate) fn import(record: &Value) -> Result<Vec<u8>> {
        let sessions =
            record["_sessions"].as_object().ok_or_else(|| Error::signal("libsignal session record has no sessions"))?;
        let mut open: Vec<(i64, SessionState)> = Vec::new();
        let mut closed: Vec<(i64, SessionState)> = Vec::new();
        for entry in sessions.values() {
            let parsed = state(entry)?;
            let closed_at = number(&entry["indexInfo"]["closed"]).unwrap_or(-1);
            let used = number(&entry["indexInfo"]["used"]).unwrap_or(0);
            if closed_at == -1 {
                open.push((used, parsed));
            } else {
                closed.push((closed_at, parsed));
            }
        }
        open.sort_by_key(|(used, _)| std::cmp::Reverse(*used));
        let mut open = open.into_iter().map(|(_, s)| s);
        let current = open.next();
        closed.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        let mut previous: Vec<SessionState> = open.chain(closed.into_iter().map(|(_, s)| s)).collect();
        previous.truncate(MAX_PREVIOUS_SESSIONS);
        Ok(SessionRecord { current, previous }.to_bytes())
    }
}

pub(crate) use libsignal::import as import_libsignal_session;

#[cfg(any(feature = "sqlite", feature = "mongo"))]
mod standard {
    use super::*;
    use crate::proto::{
        SignalChain, SignalChainKey, SignalMessageKeys, SignalPendingPreKey, SignalRecordStructure,
        SignalSessionStructure,
    };

    fn public(raw: Option<&[u8]>, what: &str) -> Result<[u8; 32]> {
        match raw {
            Some(bytes) if bytes.len() == 33 => strip_key_prefix(bytes),
            Some(bytes) => array32(bytes),
            None => Err(Error::signal(format!("signal record has no {what}"))),
        }
    }

    fn secret(raw: Option<&[u8]>, what: &str) -> Result<[u8; 32]> {
        array32(raw.ok_or_else(|| Error::signal(format!("signal record has no {what}")))?)
    }

    fn chain_key(chain: &SignalChain) -> (i64, Option<[u8; 32]>) {
        let key = chain.chain_key.as_ref();
        let index = key.and_then(|k| k.index).unwrap_or(0);
        let bytes = key.and_then(|k| k.key.as_deref()).and_then(|k| array32(k).ok()).filter(|k| *k != [0; 32]);
        (i64::from(index) - 1, bytes)
    }

    fn receiving(chain: &SignalChain) -> Result<Chain> {
        let (counter, key) = chain_key(chain);
        let derived = chain
            .message_keys
            .iter()
            .map(|k| {
                let iv = k.iv.as_deref().unwrap_or_default();
                Ok((
                    k.index.unwrap_or(0),
                    MessageSecrets {
                        cipher: secret(k.cipher_key.as_deref(), "message cipher key")?,
                        mac: secret(k.mac_key.as_deref(), "message mac key")?,
                        iv: iv.try_into().map_err(|_| Error::signal("message iv has the wrong length"))?,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        Ok(Chain {
            ratchet: public(chain.sender_ratchet_key.as_deref(), "receiver ratchet key")?,
            key,
            counter,
            skipped: Vec::new(),
            derived,
        })
    }

    pub(super) fn state_from(structure: &SignalSessionStructure) -> Result<SessionState> {
        let sender =
            structure.sender_chain.as_ref().ok_or_else(|| Error::signal("signal record has no sender chain"))?;
        let ours = KeyPair::from_private(secret(sender.sender_ratchet_key_private.as_deref(), "ratchet private key")?);
        if ours.public != public(sender.sender_ratchet_key.as_deref(), "ratchet public key")? {
            return Err(Error::signal("signal record has an inconsistent ratchet key pair"));
        }
        let (counter, key) = chain_key(sender);
        let sending_ratchet = ours.public;
        let mut receiving_chains = structure.receiver_chains.iter().map(receiving).collect::<Result<Vec<_>>>()?;
        if receiving_chains.len() > MAX_RECEIVING_CHAINS {
            receiving_chains.drain(..receiving_chains.len() - MAX_RECEIVING_CHAINS);
        }
        let pending = structure
            .pending_pre_key
            .as_ref()
            .map(|p| {
                Ok::<_, Error>(PendingPreKey {
                    pre_key_id: p.pre_key_id,
                    signed_pre_key_id: p.signed_pre_key_id.unwrap_or(0) as u32,
                    base_key: public(p.base_key.as_deref(), "pending base key")?,
                })
            })
            .transpose()?;
        let base_key = match structure.alice_base_key.as_deref() {
            Some(raw) => public(Some(raw), "base key")?,
            None => pending.as_ref().map_or([0; 32], |p| p.base_key),
        };
        Ok(SessionState {
            registration_id: structure.remote_registration_id.unwrap_or(0),
            root_key: secret(structure.root_key.as_deref(), "root key")?,
            their_ratchet: receiving_chains.last().map_or([0; 32], |c| c.ratchet),
            our_ratchet: ours,
            previous_counter: structure.previous_counter.unwrap_or(0),
            sending: Some(Chain { ratchet: sending_ratchet, key, counter, skipped: Vec::new(), derived: Vec::new() }),
            receiving: receiving_chains,
            remote_identity: public(structure.remote_identity_public.as_deref(), "remote identity")?,
            base_key,
            pending,
        })
    }

    fn chain_to(chain: &Chain, private: Option<&[u8; 32]>) -> SignalChain {
        let mut message_keys: Vec<SignalMessageKeys> = chain
            .skipped
            .iter()
            .map(|(index, seed)| (*index, MessageSecrets::from_seed(seed)))
            .chain(chain.derived.iter().copied())
            .map(|(index, secrets)| SignalMessageKeys {
                index: Some(index),
                cipher_key: Some(secrets.cipher.to_vec()),
                mac_key: Some(secrets.mac.to_vec()),
                iv: Some(secrets.iv.to_vec()),
            })
            .collect();
        message_keys.sort_by_key(|k| k.index);
        SignalChain {
            sender_ratchet_key: Some(signal_key(&chain.ratchet).to_vec()),
            sender_ratchet_key_private: private.map(|p| p.to_vec()),
            chain_key: Some(SignalChainKey {
                index: Some((chain.counter + 1).max(0) as u32),
                key: Some(chain.key.unwrap_or([0; 32]).to_vec()),
            }),
            message_keys,
        }
    }

    pub(super) fn state_to(
        state: &SessionState,
        identity: &KeyPair,
        registration_id: u32,
    ) -> Option<SignalSessionStructure> {
        let sending = state.sending.as_ref()?;
        Some(SignalSessionStructure {
            session_version: Some(3),
            local_identity_public: Some(identity.signal_public().to_vec()),
            remote_identity_public: Some(signal_key(&state.remote_identity).to_vec()),
            root_key: Some(state.root_key.to_vec()),
            previous_counter: Some(state.previous_counter),
            sender_chain: Some(chain_to(sending, Some(&state.our_ratchet.private))),
            receiver_chains: state.receiving.iter().map(|chain| chain_to(chain, None)).collect(),
            pending_pre_key: state.pending.as_ref().map(|p| SignalPendingPreKey {
                pre_key_id: p.pre_key_id,
                base_key: Some(signal_key(&p.base_key).to_vec()),
                signed_pre_key_id: Some(p.signed_pre_key_id as i32),
            }),
            remote_registration_id: Some(state.registration_id),
            local_registration_id: Some(registration_id),
            needs_refresh: None,
            alice_base_key: Some(signal_key(&state.base_key).to_vec()),
        })
    }

    pub(crate) fn import(bytes: &[u8]) -> Result<Vec<u8>> {
        let record = SignalRecordStructure::decode(bytes)?;
        let current = record.current_session.as_ref().map(state_from).transpose()?;
        let mut previous = record.previous_sessions.iter().filter_map(|s| state_from(s).ok()).collect::<Vec<_>>();
        previous.truncate(MAX_PREVIOUS_SESSIONS);
        Ok(SessionRecord { current, previous }.to_bytes())
    }

    pub(crate) fn export(ours: &[u8], identity: &KeyPair, registration_id: u32) -> Result<Vec<u8>> {
        let record = SessionRecord::from_bytes(ours)?;
        let current = record
            .current
            .as_ref()
            .and_then(|s| state_to(s, identity, registration_id))
            .ok_or_else(|| Error::signal("session has no sending chain to export"))?;
        Ok(SignalRecordStructure {
            current_session: Some(current),
            previous_sessions: record.previous.iter().filter_map(|s| state_to(s, identity, registration_id)).collect(),
        }
        .encode_to_vec())
    }

    pub(crate) fn remote_identity(ours: &[u8]) -> Option<[u8; 32]> {
        SessionRecord::from_bytes(ours).ok()?.current.map(|s| s.remote_identity)
    }
}

#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub(crate) use standard::{
    export as export_standard_session, import as import_standard_session, remote_identity as session_remote_identity,
};

fn chain_to_proto(chain: &Chain) -> StoredChain {
    StoredChain {
        ratchet_key: chain.ratchet.to_vec(),
        chain_key: chain.key.map(|k| k.to_vec()),
        counter: chain.counter,
        message_keys: chain
            .skipped
            .iter()
            .map(|(index, key)| StoredMessageKey { index: *index, key: key.to_vec() })
            .collect(),
        derived_keys: chain
            .derived
            .iter()
            .map(|(index, secrets)| StoredDerivedKey {
                index: *index,
                cipher_key: secrets.cipher.to_vec(),
                mac_key: secrets.mac.to_vec(),
                iv: secrets.iv.to_vec(),
            })
            .collect(),
    }
}

fn chain_from_proto(chain: StoredChain) -> Result<Chain> {
    Ok(Chain {
        ratchet: array32(&chain.ratchet_key)?,
        key: chain.chain_key.as_deref().map(array32).transpose()?,
        counter: chain.counter,
        skipped: chain.message_keys.into_iter().map(|k| Ok((k.index, array32(&k.key)?))).collect::<Result<_>>()?,
        derived: chain
            .derived_keys
            .into_iter()
            .map(|k| {
                Ok((
                    k.index,
                    MessageSecrets {
                        cipher: array32(&k.cipher_key)?,
                        mac: array32(&k.mac_key)?,
                        iv: k.iv.as_slice().try_into().map_err(|_| Error::signal("stored iv has the wrong length"))?,
                    },
                ))
            })
            .collect::<Result<_>>()?,
    })
}

fn state_to_proto(state: &SessionState) -> StoredSessionState {
    StoredSessionState {
        registration_id: state.registration_id,
        root_key: state.root_key.to_vec(),
        our_ratchet_private: state.our_ratchet.private.to_vec(),
        our_ratchet_public: state.our_ratchet.public.to_vec(),
        their_ratchet: state.their_ratchet.to_vec(),
        previous_counter: state.previous_counter,
        sending: state.sending.as_ref().map(chain_to_proto),
        receiving: state.receiving.iter().map(chain_to_proto).collect(),
        remote_identity: state.remote_identity.to_vec(),
        base_key: state.base_key.to_vec(),
        pending: state.pending.as_ref().map(|p| StoredPendingPreKey {
            pre_key_id: p.pre_key_id,
            signed_pre_key_id: p.signed_pre_key_id,
            base_key: p.base_key.to_vec(),
        }),
    }
}

fn state_from_proto(stored: StoredSessionState) -> Result<SessionState> {
    let private = array32(&stored.our_ratchet_private)?;
    // Sessions saved before 0.2 only kept the private half. Rebuilding the public one is a full curve
    // multiplication (~40 µs), so only pay it for those.
    let our_ratchet = match stored.our_ratchet_public.as_slice().try_into() {
        Ok(public) => KeyPair { public, private },
        Err(_) => KeyPair::from_private(private),
    };
    Ok(SessionState {
        registration_id: stored.registration_id,
        root_key: array32(&stored.root_key)?,
        our_ratchet,
        their_ratchet: array32(&stored.their_ratchet)?,
        previous_counter: stored.previous_counter,
        sending: stored.sending.map(chain_from_proto).transpose()?,
        receiving: stored.receiving.into_iter().map(chain_from_proto).collect::<Result<_>>()?,
        remote_identity: array32(&stored.remote_identity)?,
        base_key: array32(&stored.base_key)?,
        pending: stored
            .pending
            .map(|p| {
                Ok::<_, Error>(PendingPreKey {
                    pre_key_id: p.pre_key_id,
                    signed_pre_key_id: p.signed_pre_key_id,
                    base_key: array32(&p.base_key)?,
                })
            })
            .transpose()?,
    })
}
