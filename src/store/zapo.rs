#[cfg(feature = "mongo")]
mod mongo;
#[cfg(feature = "sqlite")]
mod sqlite;

use std::collections::{BTreeMap, HashSet};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use prost::Message as _;

use super::{Creds, KeyKind, SignedPreKey, Store};
use crate::crypto::{signal_key, strip_key_prefix, KeyPair};
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::proto::{StoredHashEntry, StoredHashState};
use crate::signal::{
    export_standard_sender_key, export_standard_session, import_standard_sender_key, import_standard_session,
    session_remote_identity,
};

#[cfg(feature = "mongo")]
pub use mongo::{export_zapo_mongo, import_zapo_mongo, ZapoMongo};
#[cfg(feature = "sqlite")]
pub use sqlite::{export_zapo_sqlite, import_zapo_sqlite, zapo_sessions};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZapoImport {
    pub me: Option<Jid>,
    pub sessions: usize,
    pub pre_keys: usize,
    pub sender_keys: usize,
    pub app_state_keys: usize,
    pub app_state_versions: usize,
    pub lid_mappings: usize,
    pub failed: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZapoExport {
    pub sessions: usize,
    pub pre_keys: usize,
    pub sender_keys: usize,
    pub app_state_keys: usize,
    pub app_state_versions: usize,
    pub failed: Vec<(String, String)>,
}

fn failed(err: impl std::fmt::Display) -> Error {
    Error::Store(format!("zapo: {err}"))
}

fn key32(raw: &[u8], what: &str) -> Result<[u8; 32]> {
    match raw.len() {
        33 => strip_key_prefix(raw),
        _ => raw.try_into().map_err(|_| failed(format!("{what} has {} bytes instead of 32", raw.len()))),
    }
}

fn key_pair(public: &[u8], private: &[u8], what: &str) -> Result<KeyPair> {
    let pair = KeyPair::from_private(key32(private, what)?);
    if key32(public, what)? != pair.public {
        return Err(failed(format!("{what} public key does not match its private key")));
    }
    Ok(pair)
}

fn address(user: &str, server: &str, device: u16) -> String {
    Jid { user: user.to_owned(), device, server: server.to_owned() }.signal_address()
}

fn split_address(address: &str) -> Option<(String, String, u16)> {
    let (left, device) = address.rsplit_once('.')?;
    let device = device.parse().ok()?;
    let (user, agent) = match left.split_once('_') {
        Some((user, agent)) => (user, agent.parse().ok()?),
        None => (left, 0),
    };
    let jid = Jid::from_agent(user.to_owned(), agent, device);
    Some((jid.user, jid.server, device))
}

fn lid_pair(a: Option<String>, b: Option<String>) -> Option<(String, String)> {
    let a: Jid = a?.parse().ok()?;
    let b: Jid = b?.parse().ok()?;
    match (a.is_pn(), b.is_pn()) {
        (true, false) if b.is_lid() => Some((a.user, b.user)),
        (false, true) if a.is_lid() => Some((b.user, a.user)),
        _ => None,
    }
}

fn key_epoch(key_id: &[u8]) -> i64 {
    match key_id {
        [_, _, a, b, c, d, ..] => i64::from(u32::from_be_bytes([*a, *b, *c, *d])),
        _ => -1,
    }
}

struct Snapshot {
    creds: Creds,
    writes: Vec<(KeyKind, String, Vec<u8>)>,
    report: ZapoImport,
}

#[derive(Default)]
struct ZapoData {
    noise: (Vec<u8>, Vec<u8>),
    registration: (u32, Vec<u8>, Vec<u8>),
    signed: (u32, Vec<u8>, Vec<u8>, Vec<u8>),
    adv_secret: Vec<u8>,
    account: Option<Vec<u8>>,
    me: Option<String>,
    lid: Option<String>,
    name: Option<String>,
    platform: Option<String>,
    routing_info: Option<Vec<u8>>,
    next_pre_key: Option<u32>,
    sessions: Vec<(String, String, u16, Vec<u8>)>,
    pre_keys: Vec<(u32, Vec<u8>)>,
    sender_keys: Vec<(String, String, String, u16, Vec<u8>)>,
    app_state_keys: Vec<(Vec<u8>, Vec<u8>)>,
    versions: Vec<(String, u64, Vec<u8>)>,
    index_values: Vec<(String, String, Vec<u8>)>,
    lid_pairs: Vec<(Option<String>, Option<String>)>,
}

impl ZapoData {
    fn creds(&mut self) -> Result<Creds> {
        let (signed_id, signed_public, signed_private, signature) = &self.signed;
        let signed_pre_key = SignedPreKey {
            id: *signed_id,
            key_pair: key_pair(signed_public, signed_private, "signed pre key")?,
            signature: signature
                .as_slice()
                .try_into()
                .map_err(|_| failed("the signed pre key signature must have 64 bytes"))?,
        };
        let mut creds = Creds::generate();
        creds.noise_key = key_pair(&self.noise.0, &self.noise.1, "noise key")?;
        creds.identity_key = key_pair(&self.registration.1, &self.registration.2, "identity key")?;
        creds.registration_id = self.registration.0;
        creds.signed_pre_key = signed_pre_key;
        creds.adv_secret_key =
            self.adv_secret.as_slice().try_into().map_err(|_| failed("the adv secret must have 32 bytes"))?;
        creds.me = self.me.take().and_then(|j| j.parse().ok());
        creds.lid = self.lid.take().and_then(|j| j.parse().ok());
        creds.push_name = self.name.take();
        creds.account = self.account.take();
        creds.registered = creds.account.is_some();
        creds.platform = self.platform.take();
        creds.routing_info = self.routing_info.take();
        creds.signed_pre_key_rotated_at = crate::message::now_secs();
        Ok(creds)
    }

    fn convert(mut self) -> Result<Snapshot> {
        let mut creds = self.creds()?;
        let mut report = ZapoImport { me: creds.me.clone(), ..Default::default() };
        let mut writes = Vec::new();

        for (user, server, device, record) in self.sessions {
            let key = address(&user, &server, device);
            match import_standard_session(&record) {
                Ok(value) => {
                    writes.push((KeyKind::Session, key, value));
                    report.sessions += 1;
                }
                Err(err) => report.failed.push((format!("session {key}"), err.to_string())),
            }
        }

        let highest = self.pre_keys.iter().map(|(id, _)| *id).max().unwrap_or(0);
        for (id, private) in self.pre_keys {
            match key32(&private, "pre key") {
                Ok(private) => {
                    writes.push((KeyKind::PreKey, id.to_string(), private.to_vec()));
                    report.pre_keys += 1;
                }
                Err(err) => report.failed.push((format!("pre key {id}"), err.to_string())),
            }
        }
        creds.next_pre_key_id = self.next_pre_key.unwrap_or(highest + 1).max(highest + 1).max(1);
        creds.first_unuploaded_pre_key_id = creds.next_pre_key_id;

        for (group, user, server, device, record) in self.sender_keys {
            let key = format!("{group}::{}", address(&user, &server, device));
            match import_standard_sender_key(&record) {
                Ok(value) => {
                    writes.push((KeyKind::SenderKey, key, value));
                    report.sender_keys += 1;
                }
                Err(err) => report.failed.push((format!("sender key {key}"), err.to_string())),
            }
        }

        self.app_state_keys.sort_by_key(|(id, _)| key_epoch(id));
        for (id, data) in self.app_state_keys {
            let id = STANDARD.encode(id);
            creds.app_state_key_id = Some(id.clone());
            writes.push((KeyKind::AppStateKey, id, data));
            report.app_state_keys += 1;
        }

        let mut entries: BTreeMap<String, Vec<StoredHashEntry>> = BTreeMap::new();
        for (collection, index, value_mac) in self.index_values {
            if let Ok(index_mac) = hex::decode(&index) {
                entries.entry(collection).or_default().push(StoredHashEntry { index_mac, value_mac });
            }
        }
        for (collection, version, hash) in self.versions {
            if hash.len() != 128 {
                report.failed.push((format!("app state {collection}"), "the hash must have 128 bytes".into()));
                continue;
            }
            let state = StoredHashState { version, hash, entries: entries.remove(&collection).unwrap_or_default() };
            writes.push((KeyKind::AppStateVersion, collection, state.encode_to_vec()));
            report.app_state_versions += 1;
        }

        let mut seen = HashSet::new();
        for (pn, lid) in self.lid_pairs.into_iter().filter_map(|(a, b)| lid_pair(a, b)) {
            if seen.insert(pn.clone()) {
                writes.push((KeyKind::LidMapping, format!("pn:{pn}"), lid.clone().into_bytes()));
                writes.push((KeyKind::LidMapping, format!("lid:{lid}"), pn.into_bytes()));
                report.lid_mappings += 1;
            }
        }
        Ok(Snapshot { creds, writes, report })
    }
}

async fn apply(snapshot: Snapshot, store: &dyn Store) -> Result<ZapoImport> {
    for (kind, key, value) in snapshot.writes {
        store.set(kind, &key, value).await?;
    }
    store.save_creds(&snapshot.creds).await?;
    store.flush().await?;
    Ok(snapshot.report)
}
type SessionRow = (String, String, u16, Vec<u8>, Option<Vec<u8>>);

#[derive(Default)]
struct Outgoing {
    sessions: Vec<SessionRow>,
    pre_keys: Vec<(u32, [u8; 32], [u8; 32], bool)>,
    sender_keys: Vec<(String, String, String, u16, Vec<u8>)>,
    app_state_keys: Vec<(Vec<u8>, Vec<u8>, bool)>,
    versions: Vec<(String, StoredHashState)>,
}

async fn collect(store: &dyn Store, creds: &Creds, report: &mut ZapoExport) -> Result<Outgoing> {
    let mut out = Outgoing::default();
    for key in store.keys(KeyKind::Session).await? {
        let Some(record) = store.get(KeyKind::Session, &key).await? else { continue };
        let outcome =
            split_address(&key).ok_or_else(|| failed("the session address is not a signal address")).and_then(
                |parts| Ok((parts, export_standard_session(&record, &creds.identity_key, creds.registration_id)?)),
            );
        match outcome {
            Ok(((user, server, device), exported)) => {
                let identity = session_remote_identity(&record).map(|k| signal_key(&k).to_vec());
                out.sessions.push((user, server, device, exported, identity));
            }
            Err(err) => report.failed.push((format!("session {key}"), err.to_string())),
        }
    }
    for key in store.keys(KeyKind::PreKey).await? {
        let (Ok(id), Some(private)) = (key.parse::<u32>(), store.get(KeyKind::PreKey, &key).await?) else { continue };
        match key32(&private, "pre key") {
            Ok(private) => {
                let pair = KeyPair::from_private(private);
                out.pre_keys.push((id, pair.public, pair.private, id < creds.first_unuploaded_pre_key_id));
            }
            Err(err) => report.failed.push((format!("pre key {key}"), err.to_string())),
        }
    }
    for key in store.keys(KeyKind::SenderKey).await? {
        let Some(record) = store.get(KeyKind::SenderKey, &key).await? else { continue };
        let outcome = key
            .rsplit_once("::")
            .and_then(|(group, sender)| Some((group.to_owned(), split_address(sender)?)))
            .ok_or_else(|| failed("the sender key name is not group::address"))
            .and_then(|parts| Ok((parts, export_standard_sender_key(&record)?)));
        match outcome {
            Ok(((group, (user, server, device)), exported)) => {
                out.sender_keys.push((group, user, server, device, exported))
            }
            Err(err) => report.failed.push((format!("sender key {key}"), err.to_string())),
        }
    }
    for key in store.keys(KeyKind::AppStateKey).await? {
        let (Ok(id), Some(data)) = (STANDARD.decode(&key), store.get(KeyKind::AppStateKey, &key).await?) else {
            continue;
        };
        let newest = creds.app_state_key_id.as_deref() == Some(key.as_str());
        out.app_state_keys.push((id, data, newest));
    }
    for name in store.keys(KeyKind::AppStateVersion).await? {
        let Some(raw) = store.get(KeyKind::AppStateVersion, &name).await? else { continue };
        match StoredHashState::decode(raw.as_slice()) {
            Ok(state) => out.versions.push((name, state)),
            Err(err) => report.failed.push((format!("app state {name}"), err.to_string())),
        }
    }
    Ok(out)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde_json::Value;

    use super::*;
    use crate::signal::{EncType, SignalRepository};
    use crate::store::MemoryStore;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("zapo")
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn kind(name: &str) -> EncType {
        match name {
            "pkmsg" => EncType::PreKey,
            _ => EncType::Message,
        }
    }

    async fn take_over(db: &Path, expected: &Value) -> (Arc<MemoryStore>, SignalRepository) {
        let store = Arc::new(MemoryStore::new());
        let report = import_zapo_sqlite(db, "bot", store.as_ref()).await.unwrap();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(
            (
                report.sessions,
                report.sender_keys,
                report.app_state_keys,
                report.app_state_versions,
                report.lid_mappings
            ),
            (1, 2, 1, 1, 1)
        );
        assert_eq!(report.pre_keys as u64, expected["preKeys"].as_u64().unwrap() - 1);

        let creds = store.load_creds().await.unwrap().unwrap();
        assert_eq!(creds.me.as_ref().map(Jid::to_string).as_deref(), expected["me"].as_str());
        assert_eq!(creds.lid.as_ref().map(Jid::to_string).as_deref(), expected["lid"].as_str());
        assert_eq!(u64::from(creds.registration_id), expected["registrationId"].as_u64().unwrap());
        assert_eq!(creds.push_name.as_deref(), Some("Bot zapo"));
        assert!(creds.is_logged_in());
        assert!(crate::crypto::verify(
            &creds.identity_key.public,
            &creds.signed_pre_key.key_pair.signal_public(),
            &creds.signed_pre_key.signature
        ));
        assert_eq!(creds.app_state_key_id.as_deref(), expected["appStateKeyId"].as_str());
        let key = store.get(KeyKind::AppStateKey, expected["appStateKeyId"].as_str().unwrap()).await.unwrap();
        assert_eq!(key.map(hex::encode).as_deref(), expected["appStateKey"].as_str());
        let version = store.get(KeyKind::AppStateVersion, "regular").await.unwrap().unwrap();
        let version = StoredHashState::decode(version.as_slice()).unwrap();
        assert_eq!(version.version, expected["appStateVersion"].as_u64().unwrap());
        assert_eq!(
            version.entries.first().map(|e| hex::encode(&e.index_mac)).as_deref(),
            expected["indexMac"].as_str()
        );
        let pair = &expected["lidPair"];
        let lid = store.get(KeyKind::LidMapping, &format!("pn:{}", pair["pn"].as_str().unwrap())).await.unwrap();
        assert_eq!(lid.as_deref(), pair["lid"].as_str().map(str::as_bytes));

        let repo = SignalRepository::new(store.clone(), &creds);
        for pending in expected["pending"].as_array().unwrap() {
            let from = expected[pending["from"].as_str().unwrap()].as_str().unwrap();
            let body = hex::decode(pending["body"].as_str().unwrap()).unwrap();
            let plain = repo.decrypt(from, kind(pending["type"].as_str().unwrap()), &body).await.unwrap();
            assert_eq!(String::from_utf8(plain).unwrap(), pending["text"].as_str().unwrap());
        }
        let group = &expected["groupPending"];
        let plain = repo
            .group_decrypt(
                expected["group"].as_str().unwrap(),
                expected["groupSender"].as_str().unwrap(),
                &hex::decode(group["body"].as_str().unwrap()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(String::from_utf8(plain).unwrap(), group["text"].as_str().unwrap());
        (store, repo)
    }

    #[tokio::test]
    async fn zapo_import_and_rollback() {
        let expected = read_json(&fixtures().join("expected.json"));
        let (store, repo) = take_over(&fixtures().join("alice.sqlite"), &expected).await;
        let bob = expected["bob"].as_str().unwrap();
        let carol = expected["carol"].as_str().unwrap();
        assert_eq!(repo.encrypt(bob, b"respuesta de soblendwa").await.unwrap().0, EncType::Message);
        assert_eq!(repo.encrypt(carol, b"hola carol").await.unwrap().0, EncType::Message);
        repo.group_encrypt(expected["group"].as_str().unwrap(), expected["self"].as_str().unwrap(), b"grupo")
            .await
            .unwrap();

        let dir = std::env::temp_dir().join(format!("soblendwa-zapo-{}", crate::crypto::random_u32()));
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("alice.sqlite");
        std::fs::copy(fixtures().join("alice.sqlite"), &copy).unwrap();
        let back = export_zapo_sqlite(store.as_ref(), &copy, "bot").await.unwrap();
        assert!(back.failed.is_empty(), "{:?}", back.failed);
        assert_eq!((back.sessions, back.sender_keys, back.app_state_keys, back.app_state_versions), (2, 2, 1, 1));
        assert_eq!(zapo_sessions(&copy).unwrap(), vec!["bot".to_owned()]);

        let again = Arc::new(MemoryStore::new());
        let report = import_zapo_sqlite(&copy, "bot", again.as_ref()).await.unwrap();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!((report.sessions, report.sender_keys), (2, 2));
        for kind in [KeyKind::Session, KeyKind::SenderKey, KeyKind::PreKey] {
            let mut ours = store.keys(kind).await.unwrap();
            let mut theirs = again.keys(kind).await.unwrap();
            ours.sort();
            theirs.sort();
            assert_eq!(ours, theirs, "{kind:?}");
        }
        let creds = again.load_creds().await.unwrap().unwrap();
        let reopened = SignalRepository::new(again.clone(), &creds);
        assert_eq!(reopened.encrypt(bob, b"sigo vivo tras el rollback").await.unwrap().0, EncType::Message);
        drop(reopened);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[ignore = "set SOBLEND_ZAPO_WORK to a folder produced by the zapo fixture script"]
    async fn zapo_takeover() {
        let work = PathBuf::from(std::env::var("SOBLEND_ZAPO_WORK").unwrap());
        let expected = read_json(&work.join("expected.json"));
        let db = work.join("alice.sqlite");
        let (store, repo) = take_over(&db, &expected).await;
        let mut pairwise = Vec::new();
        for (to, text) in [
            ("bob", "soblendwa contesta a bob"),
            ("bob", "segunda respuesta a bob"),
            ("carol", "soblendwa contesta a carol"),
        ] {
            let (kind, body) = repo.encrypt(expected[to].as_str().unwrap(), text.as_bytes()).await.unwrap();
            pairwise
                .push(serde_json::json!({ "to": to, "type": kind.as_str(), "body": hex::encode(body), "text": text }));
        }
        let group_text = "grupo: soblendwa habla con la sender key de zapo";
        let (skmsg, _) = repo
            .group_encrypt(
                expected["group"].as_str().unwrap(),
                expected["self"].as_str().unwrap(),
                group_text.as_bytes(),
            )
            .await
            .unwrap();
        let replies = serde_json::json!({
            "pairwise": pairwise,
            "group": { "body": hex::encode(skmsg), "text": group_text },
        });
        std::fs::write(work.join("rust_replies.json"), serde_json::to_vec_pretty(&replies).unwrap()).unwrap();
        let back = export_zapo_sqlite(store.as_ref(), &db, "bot").await.unwrap();
        assert!(back.failed.is_empty(), "{:?}", back.failed);
    }

    #[test]
    fn address_roundtrip() {
        for (user, server, device, ours) in [
            ("5215511111111", "s.whatsapp.net", 0, "5215511111111.0"),
            ("111111111111111", "lid", 7, "111111111111111_1.7"),
            ("5215522222222", "hosted", 99, "5215522222222_128.99"),
        ] {
            assert_eq!(address(user, server, device), ours);
            assert_eq!(split_address(ours), Some((user.to_owned(), server.to_owned(), device)));
        }
        assert_eq!(
            lid_pair(Some("222222222222222@lid".into()), Some("5215522222222@s.whatsapp.net".into())),
            Some(("5215522222222".into(), "222222222222222".into()))
        );
        assert_eq!(lid_pair(Some("5215522222222@s.whatsapp.net".into()), None), None);
    }
}
