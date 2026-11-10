#[cfg(test)]
mod bench;
mod group;
mod session;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

pub use session::{EncType, PreKeyBundle};

use group::SenderKeyRecord;
use session::{IncomingPreKey, SessionRecord, SessionState};

use crate::crypto::KeyPair;
use crate::error::{Error, Result};
use crate::store::{Creds, KeyKind, Store};

#[derive(Default)]
struct Cache {
    sessions: HashMap<String, SessionRecord>,
    sender_keys: HashMap<String, SenderKeyRecord>,
}

pub(crate) struct SignalRepository {
    store: Arc<dyn Store>,
    identity: KeyPair,
    registration_id: u32,
    signed_pre_key: parking_lot::RwLock<(u32, KeyPair)>,
    cache: Mutex<Cache>,
}

const PARALLEL_THRESHOLD: usize = 16;

pub(crate) fn json_bytes(value: &serde_json::Value) -> Option<Vec<u8>> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
    use base64::Engine;
    use serde_json::Value;
    match value {
        Value::String(text) => STANDARD.decode(text).or_else(|_| STANDARD_NO_PAD.decode(text)).ok(),
        Value::Array(items) => items.iter().map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok())).collect(),
        Value::Object(map) => match (map.get("type").and_then(Value::as_str), map.get("data")) {
            (Some("Buffer"), Some(data)) => json_bytes(data),
            _ => {
                let mut indexed: Vec<(usize, u8)> = map
                    .iter()
                    .map(|(k, v)| Some((k.parse().ok()?, v.as_u64().and_then(|n| u8::try_from(n).ok())?)))
                    .collect::<Option<_>>()?;
                indexed.sort_by_key(|(index, _)| *index);
                let dense = !indexed.is_empty() && indexed.iter().enumerate().all(|(n, (index, _))| n == *index);
                dense.then(|| indexed.into_iter().map(|(_, byte)| byte).collect())
            }
        },
        _ => None,
    }
}

pub(crate) fn import_libsignal_session(record: &serde_json::Value) -> Result<Vec<u8>> {
    session::import_libsignal_session(record)
}

pub(crate) fn import_baileys_sender_key(record: &serde_json::Value) -> Result<Vec<u8>> {
    SenderKeyRecord::from_baileys(record).map(|r| r.to_bytes())
}

#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub(crate) fn import_standard_session(record: &[u8]) -> Result<Vec<u8>> {
    session::import_standard_session(record)
}

#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub(crate) fn export_standard_session(ours: &[u8], identity: &KeyPair, registration_id: u32) -> Result<Vec<u8>> {
    session::export_standard_session(ours, identity, registration_id)
}

#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub(crate) fn session_remote_identity(ours: &[u8]) -> Option<[u8; 32]> {
    session::session_remote_identity(ours)
}

#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub(crate) fn import_standard_sender_key(record: &[u8]) -> Result<Vec<u8>> {
    SenderKeyRecord::from_standard(record).map(|r| r.to_bytes())
}

#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub(crate) fn export_standard_sender_key(ours: &[u8]) -> Result<Vec<u8>> {
    SenderKeyRecord::from_bytes(ours).map(|r| r.to_standard())
}

fn sender_key_name(group: &str, sender: &str) -> String {
    format!("{group}::{sender}")
}

type Initiated = Vec<(String, Result<SessionState>)>;

fn initiate_all(identity: &KeyPair, bundles: &[(String, PreKeyBundle)]) -> Initiated {
    bundles.iter().map(|(address, bundle)| (address.clone(), SessionState::initiate(identity, bundle))).collect()
}

fn initiate_parallel(identity: &KeyPair, bundles: &[(String, PreKeyBundle)]) -> Initiated {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(bundles.len()).max(1);
    let chunk = bundles.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let workers: Vec<_> =
            bundles.chunks(chunk).map(|part| scope.spawn(move || initiate_all(identity, part))).collect();
        workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
    })
}

impl SignalRepository {
    pub(crate) fn new(store: Arc<dyn Store>, creds: &Creds) -> Self {
        SignalRepository {
            store,
            identity: creds.identity_key.clone(),
            registration_id: creds.registration_id,
            signed_pre_key: parking_lot::RwLock::new((creds.signed_pre_key.id, creds.signed_pre_key.key_pair.clone())),
            cache: Mutex::new(Cache::default()),
        }
    }

    async fn load_session<'a>(&self, cache: &'a mut Cache, address: &str) -> Result<&'a mut SessionRecord> {
        if !cache.sessions.contains_key(address) {
            let record = match self.store.get(KeyKind::Session, address).await? {
                Some(bytes) => SessionRecord::from_bytes(&bytes).unwrap_or_default(),
                None => SessionRecord::default(),
            };
            cache.sessions.insert(address.to_owned(), record);
        }
        Ok(cache.sessions.get_mut(address).expect("inserted above"))
    }

    async fn load_sender_key<'a>(&self, cache: &'a mut Cache, name: &str) -> Result<&'a mut SenderKeyRecord> {
        if !cache.sender_keys.contains_key(name) {
            let record = match self.store.get(KeyKind::SenderKey, name).await? {
                Some(bytes) => SenderKeyRecord::from_bytes(&bytes).unwrap_or_default(),
                None => SenderKeyRecord::default(),
            };
            cache.sender_keys.insert(name.to_owned(), record);
        }
        Ok(cache.sender_keys.get_mut(name).expect("inserted above"))
    }

    pub(crate) async fn missing_sessions(&self, addresses: &[String]) -> Result<Vec<String>> {
        let mut cache = self.cache.lock().await;
        let mut missing = Vec::new();
        for address in addresses {
            if !self.load_session(&mut cache, address).await?.has_session() {
                missing.push(address.clone());
            }
        }
        Ok(missing)
    }

    pub(crate) async fn inject_bundle(&self, address: &str, bundle: &PreKeyBundle) -> Result<()> {
        let mut results = self.inject_bundles(vec![(address.to_owned(), bundle.clone())]).await;
        results.pop().map(|(_, r)| r).unwrap_or(Ok(()))
    }

    pub(crate) async fn inject_bundles(&self, bundles: Vec<(String, PreKeyBundle)>) -> Vec<(String, Result<()>)> {
        let identity = self.identity.clone();
        let initiated = if bundles.len() < PARALLEL_THRESHOLD {
            initiate_all(&identity, &bundles)
        } else {
            tokio::task::spawn_blocking(move || initiate_parallel(&identity, &bundles)).await.unwrap_or_default()
        };

        let mut cache = self.cache.lock().await;
        let mut out = Vec::with_capacity(initiated.len());
        for (address, state) in initiated {
            let result = match state {
                Ok(state) => match self.load_session(&mut cache, &address).await {
                    Ok(record) => {
                        record.promote(state);
                        self.store.set(KeyKind::Session, &address, record.to_bytes()).await
                    }
                    Err(err) => Err(err),
                },
                Err(err) => Err(err),
            };
            out.push((address, result));
        }
        out
    }

    pub(crate) async fn encrypt_many(
        &self,
        addresses: &[String],
        plaintext: &[u8],
    ) -> Vec<(String, Result<(EncType, Vec<u8>)>)> {
        let mut cache = self.cache.lock().await;
        let mut out = Vec::with_capacity(addresses.len());
        for address in addresses {
            let result = async {
                let record = self.load_session(&mut cache, address).await?;
                let encrypted = record.encrypt(&self.identity, self.registration_id, plaintext)?;
                let bytes = record.to_bytes();
                self.store.set(KeyKind::Session, address, bytes).await?;
                Ok(encrypted)
            }
            .await;
            out.push((address.clone(), result));
        }
        out
    }

    pub(crate) async fn encrypt(&self, address: &str, plaintext: &[u8]) -> Result<(EncType, Vec<u8>)> {
        let mut results = self.encrypt_many(&[address.to_owned()], plaintext).await;
        results.pop().map(|(_, r)| r).unwrap_or_else(|| Err(Error::signal("encryption produced no output")))
    }

    pub(crate) async fn decrypt(&self, address: &str, kind: EncType, data: &[u8]) -> Result<Vec<u8>> {
        let mut cache = self.cache.lock().await;
        match kind {
            EncType::Message => {
                let record = self.load_session(&mut cache, address).await?;
                let plaintext = record.decrypt(&self.identity, data)?;
                let bytes = record.to_bytes();
                drop(cache);
                self.store.set(KeyKind::Session, address, bytes).await?;
                Ok(plaintext)
            }
            EncType::PreKey => {
                let incoming = IncomingPreKey::parse(data)?;
                let signed = self.signed_pre_key_for(incoming.signed_pre_key_id).await?;
                let known = self.load_session(&mut cache, address).await?.has_base_key(&incoming.base_key);
                let one_time = match (known, incoming.pre_key_id) {
                    (false, Some(id)) => Some(
                        self.load_pre_key(id)
                            .await?
                            .ok_or_else(|| Error::signal(format!("pre-key {id} is unknown or was already used")))?,
                    ),
                    _ => None,
                };
                let record = self.load_session(&mut cache, address).await?;
                let plaintext = record.decrypt_pre_key(&self.identity, &signed, one_time.as_ref(), &incoming)?;
                let bytes = record.to_bytes();
                drop(cache);
                self.store.set(KeyKind::Session, address, bytes).await?;
                if let (Some(id), Some(_)) = (incoming.pre_key_id, one_time) {
                    self.store.remove(KeyKind::PreKey, &id.to_string()).await?;
                }
                Ok(plaintext)
            }
        }
    }

    async fn signed_pre_key_for(&self, id: Option<u32>) -> Result<KeyPair> {
        let (current_id, current) = self.signed_pre_key.read().clone();
        match id {
            Some(id) if id != current_id => match self.store.get(KeyKind::PreKey, &format!("signed:{id}")).await? {
                Some(bytes) => {
                    let private: [u8; 32] =
                        bytes.as_slice().try_into().map_err(|_| Error::signal("stored signed pre-key is corrupt"))?;
                    Ok(KeyPair::from_private(private))
                }
                None => Ok(current),
            },
            _ => Ok(current),
        }
    }

    pub(crate) async fn rotate_signed_pre_key(&self, id: u32, pair: KeyPair) -> Result<()> {
        let (previous_id, previous) = std::mem::replace(&mut *self.signed_pre_key.write(), (id, pair));
        self.store.set(KeyKind::PreKey, &format!("signed:{previous_id}"), previous.private.to_vec()).await
    }

    pub(crate) async fn load_pre_key(&self, id: u32) -> Result<Option<KeyPair>> {
        let Some(bytes) = self.store.get(KeyKind::PreKey, &id.to_string()).await? else {
            return Ok(None);
        };
        let private: [u8; 32] = bytes.as_slice().try_into().map_err(|_| Error::signal("stored pre-key is corrupt"))?;
        Ok(Some(KeyPair::from_private(private)))
    }

    pub(crate) async fn store_pre_keys(&self, keys: &[(u32, KeyPair)]) -> Result<()> {
        for (id, pair) in keys {
            self.store.set(KeyKind::PreKey, &id.to_string(), pair.private.to_vec()).await?;
        }
        Ok(())
    }

    pub(crate) async fn group_encrypt(&self, group: &str, me: &str, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let name = sender_key_name(group, me);
        let mut cache = self.cache.lock().await;
        let record = self.load_sender_key(&mut cache, &name).await?;
        if record.is_empty() {
            *record = SenderKeyRecord::create_own();
        }
        let distribution = record.distribution_message()?;
        let ciphertext = record.encrypt(plaintext)?;
        let bytes = record.to_bytes();
        drop(cache);
        self.store.set(KeyKind::SenderKey, &name, bytes).await?;
        Ok((ciphertext, distribution))
    }

    pub(crate) async fn group_distribution(&self, group: &str, me: &str) -> Result<Option<Vec<u8>>> {
        let name = sender_key_name(group, me);
        let mut cache = self.cache.lock().await;
        let record = self.load_sender_key(&mut cache, &name).await?;
        if record.is_empty() {
            return Ok(None);
        }
        record.distribution_message().map(Some)
    }

    pub(crate) async fn group_decrypt(&self, group: &str, sender: &str, data: &[u8]) -> Result<Vec<u8>> {
        let name = sender_key_name(group, sender);
        let mut cache = self.cache.lock().await;
        let record = self.load_sender_key(&mut cache, &name).await?;
        let plaintext = record.decrypt(data)?;
        let bytes = record.to_bytes();
        drop(cache);
        self.store.set(KeyKind::SenderKey, &name, bytes).await?;
        Ok(plaintext)
    }

    pub(crate) async fn process_distribution(&self, group: &str, sender: &str, data: &[u8]) -> Result<()> {
        let name = sender_key_name(group, sender);
        let mut cache = self.cache.lock().await;
        let record = self.load_sender_key(&mut cache, &name).await?;
        record.process_distribution(data)?;
        let bytes = record.to_bytes();
        drop(cache);
        self.store.set(KeyKind::SenderKey, &name, bytes).await
    }

    pub(crate) async fn reset_own_sender_key(&self, group: &str, me: &str) -> Result<()> {
        let name = sender_key_name(group, me);
        self.cache.lock().await.sender_keys.remove(&name);
        self.store.remove(KeyKind::SenderKey, &name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    struct Peer {
        repo: SignalRepository,
        creds: Creds,
    }

    async fn peer() -> Peer {
        let creds = Creds::generate();
        let repo = SignalRepository::new(Arc::new(MemoryStore::new()), &creds);
        Peer { repo, creds }
    }

    async fn bundle_for(peer: &Peer, pre_key_id: u32) -> PreKeyBundle {
        let one_time = KeyPair::generate();
        peer.repo.store_pre_keys(&[(pre_key_id, one_time.clone())]).await.unwrap();
        PreKeyBundle {
            registration_id: peer.creds.registration_id,
            identity_key: peer.creds.identity_key.public,
            signed_pre_key_id: peer.creds.signed_pre_key.id,
            signed_pre_key: peer.creds.signed_pre_key.key_pair.public,
            signed_pre_key_signature: peer.creds.signed_pre_key.signature,
            pre_key: Some((pre_key_id, one_time.public)),
        }
    }

    #[tokio::test]
    async fn ratchet_steps() {
        let alice = peer().await;
        let bob = peer().await;
        alice.repo.inject_bundle("bob.0", &bundle_for(&bob, 42).await).await.unwrap();

        let (kind, first) = alice.repo.encrypt("bob.0", b"hola bob").await.unwrap();
        assert_eq!(kind, EncType::PreKey);
        let (kind, second) = alice.repo.encrypt("bob.0", b"otra vez").await.unwrap();
        assert_eq!(kind, EncType::PreKey);

        assert_eq!(bob.repo.decrypt("alice.0", EncType::PreKey, &second).await.unwrap(), b"otra vez");
        assert_eq!(bob.repo.decrypt("alice.0", EncType::PreKey, &first).await.unwrap(), b"hola bob");
        assert!(bob.repo.load_pre_key(42).await.unwrap().is_none());

        let (kind, reply) = bob.repo.encrypt("alice.0", b"hola alice").await.unwrap();
        assert_eq!(kind, EncType::Message);
        assert_eq!(alice.repo.decrypt("bob.0", EncType::Message, &reply).await.unwrap(), b"hola alice");

        for round in 0..20u8 {
            let text = vec![round; 10 + round as usize];
            let (kind, msg) = alice.repo.encrypt("bob.0", &text).await.unwrap();
            assert_eq!(kind, EncType::Message);
            assert_eq!(bob.repo.decrypt("alice.0", kind, &msg).await.unwrap(), text);
            let (_, back) = bob.repo.encrypt("alice.0", &text).await.unwrap();
            assert_eq!(alice.repo.decrypt("bob.0", EncType::Message, &back).await.unwrap(), text);
        }
    }

    #[tokio::test]
    async fn out_of_order() {
        let alice = peer().await;
        let bob = peer().await;
        alice.repo.inject_bundle("bob.0", &bundle_for(&bob, 1).await).await.unwrap();
        let (_, hello) = alice.repo.encrypt("bob.0", b"hello").await.unwrap();
        bob.repo.decrypt("alice.0", EncType::PreKey, &hello).await.unwrap();
        let (_, ack) = bob.repo.encrypt("alice.0", b"ack").await.unwrap();
        alice.repo.decrypt("bob.0", EncType::Message, &ack).await.unwrap();

        let mut queued = Vec::new();
        for i in 0..5u8 {
            queued.push((i, alice.repo.encrypt("bob.0", &[i]).await.unwrap().1));
        }
        queued.reverse();
        for (i, msg) in queued {
            assert_eq!(bob.repo.decrypt("alice.0", EncType::Message, &msg).await.unwrap(), vec![i]);
        }
    }

    #[tokio::test]
    async fn sender_keys_roundtrip() {
        let alice = peer().await;
        let bob = peer().await;
        let (first, distribution) = alice.repo.group_encrypt("group@g.us", "alice.0", b"primero").await.unwrap();
        let (second, _) = alice.repo.group_encrypt("group@g.us", "alice.0", b"segundo").await.unwrap();
        bob.repo.process_distribution("group@g.us", "alice.0", &distribution).await.unwrap();
        assert_eq!(bob.repo.group_decrypt("group@g.us", "alice.0", &second).await.unwrap(), b"segundo");
        assert_eq!(bob.repo.group_decrypt("group@g.us", "alice.0", &first).await.unwrap(), b"primero");
        assert!(bob.repo.group_decrypt("group@g.us", "alice.0", &first).await.is_err());
    }

    #[tokio::test]
    async fn session_serde() {
        let alice = peer().await;
        let bob = peer().await;
        alice.repo.inject_bundle("bob.0", &bundle_for(&bob, 5).await).await.unwrap();
        let (_, msg) = alice.repo.encrypt("bob.0", b"persist me").await.unwrap();
        bob.repo.decrypt("alice.0", EncType::PreKey, &msg).await.unwrap();

        let restored = SignalRepository::new(bob.repo.store.clone(), &bob.creds);
        let (_, next) = alice.repo.encrypt("bob.0", b"after restart").await.unwrap();
        assert_eq!(restored.decrypt("alice.0", EncType::PreKey, &next).await.unwrap(), b"after restart");

        use prost::Message as _;
        let saved = alice.repo.store.get(KeyKind::Session, "bob.0").await.unwrap().unwrap();
        let mut old_format = crate::proto::StoredSession::decode(saved.as_slice()).unwrap();
        old_format.current.as_mut().unwrap().our_ratchet_public.clear();
        let old = SessionRecord::from_bytes(&old_format.encode_to_vec()).unwrap();
        let new = SessionRecord::from_bytes(&saved).unwrap();
        assert_eq!(
            old.to_bytes(),
            new.to_bytes(),
            "sessions saved before the public key was stored must load the same"
        );
    }
}
