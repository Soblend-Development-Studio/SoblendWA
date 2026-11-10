use std::path::Path;

use prost::Message as _;
use serde_json::Value;

use super::{Creds, KeyKind, SignedPreKey, Store};
use crate::crypto::{strip_key_prefix, KeyPair};
use crate::error::{Error, Result};
use crate::jid::Jid;
use crate::proto::{AdvSignedDeviceIdentity, StoredHashEntry, StoredHashState};
use crate::signal::{import_baileys_sender_key, import_libsignal_session, json_bytes};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BaileysImport {
    pub me: Option<Jid>,
    pub sessions: usize,
    pub pre_keys: usize,
    pub sender_keys: usize,
    pub app_state_keys: usize,
    pub app_state_versions: usize,
    pub lid_mappings: usize,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

fn invalid(what: &str) -> Error {
    Error::Store(format!("baileys creds.json: {what}"))
}

fn bytes(value: &Value, what: &str) -> Result<Vec<u8>> {
    json_bytes(value).ok_or_else(|| invalid(&format!("{what} is missing or unreadable")))
}

fn key32(value: &Value, what: &str) -> Result<[u8; 32]> {
    let raw = bytes(value, what)?;
    match raw.len() {
        33 => strip_key_prefix(&raw),
        _ => raw.as_slice().try_into().map_err(|_| invalid(&format!("{what} has {} bytes instead of 32", raw.len()))),
    }
}

fn key_pair(value: &Value, what: &str) -> Result<KeyPair> {
    let pair = KeyPair::from_private(key32(&value["private"], what)?);
    if let Ok(public) = key32(&value["public"], what) {
        if public != pair.public {
            return Err(invalid(&format!("{what} public key does not match its private key")));
        }
    }
    Ok(pair)
}

fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn optional_bytes(value: &Value) -> Option<Vec<u8>> {
    (!value.is_null()).then(|| json_bytes(value)).flatten()
}

fn plain_user(text: &str) -> String {
    let user = text.split('@').next().unwrap_or(text);
    user.split(':').next().unwrap_or(user).to_owned()
}

pub fn creds_from_baileys(value: &Value) -> Result<Creds> {
    let identity_key = key_pair(&value["signedIdentityKey"], "signedIdentityKey")?;
    let signed = &value["signedPreKey"];
    let signature: [u8; 64] = bytes(&signed["signature"], "signedPreKey.signature")?
        .try_into()
        .map_err(|_| invalid("signedPreKey.signature must have 64 bytes"))?;
    let signed_pre_key = SignedPreKey {
        id: number(&signed["keyId"]).ok_or_else(|| invalid("signedPreKey.keyId is missing"))? as u32,
        key_pair: key_pair(&signed["keyPair"], "signedPreKey.keyPair")?,
        signature,
    };
    let adv_secret_key: [u8; 32] = bytes(&value["advSecretKey"], "advSecretKey")?
        .try_into()
        .map_err(|_| invalid("advSecretKey must have 32 bytes"))?;
    let account = value.get("account").filter(|a| a.is_object()).map(|account| {
        AdvSignedDeviceIdentity {
            details: optional_bytes(&account["details"]),
            account_signature_key: optional_bytes(&account["accountSignatureKey"]),
            account_signature: optional_bytes(&account["accountSignature"]),
            device_signature: optional_bytes(&account["deviceSignature"]),
        }
        .encode_to_vec()
    });
    let me = &value["me"];
    let parse_jid = |field: &str| me[field].as_str().and_then(|j| j.parse::<Jid>().ok());
    let mut creds = Creds {
        noise_key: key_pair(&value["noiseKey"], "noiseKey")?,
        pairing_ephemeral_key: key_pair(&value["pairingEphemeralKeyPair"], "pairingEphemeralKeyPair")
            .unwrap_or_else(|_| KeyPair::generate()),
        identity_key,
        signed_pre_key,
        registration_id: number(&value["registrationId"]).ok_or_else(|| invalid("registrationId is missing"))? as u32,
        adv_secret_key,
        next_pre_key_id: number(&value["nextPreKeyId"]).unwrap_or(1) as u32,
        first_unuploaded_pre_key_id: number(&value["firstUnuploadedPreKeyId"]).unwrap_or(1) as u32,
        me: parse_jid("id"),
        lid: parse_jid("lid"),
        push_name: me["name"].as_str().map(str::to_owned),
        registered: account.is_some(),
        account,
        platform: value["platform"].as_str().map(str::to_owned),
        business_name: None,
        pairing_code: value["pairingCode"].as_str().map(str::to_owned),
        routing_info: optional_bytes(&value["routingInfo"]),
        wa_version: None,
        app_state_key_id: value["myAppStateKeyId"].as_str().map(str::to_owned),
        signed_pre_key_rotated_at: crate::message::now_secs(),
    };
    if creds.next_pre_key_id < creds.first_unuploaded_pre_key_id {
        creds.next_pre_key_id = creds.first_unuploaded_pre_key_id;
    }
    Ok(creds)
}

fn hash_state(value: &Value) -> Result<Vec<u8>> {
    let hash = bytes(&value["hash"], "app state hash")?;
    if hash.len() != 128 {
        return Err(Error::Store("app state hash must have 128 bytes".into()));
    }
    let entries = value["indexValueMap"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(index, entry)| {
            Some(StoredHashEntry {
                index_mac: json_bytes(&Value::String(index.clone()))?,
                value_mac: json_bytes(&entry["valueMac"])?,
            })
        })
        .collect();
    Ok(StoredHashState { version: number(&value["version"]).unwrap_or(0), hash, entries }.encode_to_vec())
}

fn file_id(name: &str) -> String {
    name.replace("__", "/")
}

pub async fn import_baileys(dir: impl AsRef<Path>, store: &dyn Store) -> Result<BaileysImport> {
    let dir = dir.as_ref();
    let creds_path = dir.join("creds.json");
    let raw = tokio::fs::read(&creds_path)
        .await
        .map_err(|err| Error::Store(format!("cannot read {}: {err}", creds_path.display())))?;
    let creds = creds_from_baileys(&serde_json::from_slice(&raw)?)?;
    let mut report = BaileysImport { me: creds.me.clone(), ..Default::default() };

    let mut entries = tokio::fs::read_dir(dir).await?;
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != "creds.json" && name.ends_with(".json") {
            files.push((name, entry.path()));
        }
    }
    files.sort();

    for (name, path) in files {
        let stem = name.trim_end_matches(".json");
        let outcome = async {
            let value: Value = serde_json::from_slice(&tokio::fs::read(&path).await?)?;
            if let Some(id) = stem.strip_prefix("pre-key-") {
                let pair = key_pair(&value, &name)?;
                store.set(KeyKind::PreKey, id, pair.private.to_vec()).await?;
                report.pre_keys += 1;
            } else if let Some(address) = stem.strip_prefix("session-") {
                let record = match &value {
                    Value::Object(map) if map.contains_key("_sessions") => value.clone(),
                    other => serde_json::from_slice(&bytes(other, &name)?)?,
                };
                store.set(KeyKind::Session, &file_id(address), import_libsignal_session(&record)?).await?;
                report.sessions += 1;
            } else if stem.starts_with("sender-key-memory-") {
                report.skipped.push(name.clone());
            } else if let Some(id) = stem.strip_prefix("sender-key-") {
                let mut parts = id.rsplitn(3, "--");
                let (Some(device), Some(sender), Some(group)) = (parts.next(), parts.next(), parts.next()) else {
                    return Err(Error::Store("unexpected sender key file name".into()));
                };
                let key = format!("{}::{sender}.{device}", file_id(group));
                store.set(KeyKind::SenderKey, &key, import_baileys_sender_key(&value)?).await?;
                report.sender_keys += 1;
            } else if let Some(id) = stem.strip_prefix("app-state-sync-key-") {
                let data = value.get("keyData").map(|k| bytes(k, &name)).unwrap_or_else(|| bytes(&value, &name))?;
                store.set(KeyKind::AppStateKey, &file_id(id), data).await?;
                report.app_state_keys += 1;
            } else if let Some(collection) = stem.strip_prefix("app-state-sync-version-") {
                store.set(KeyKind::AppStateVersion, collection, hash_state(&value)?).await?;
                report.app_state_versions += 1;
            } else if let Some(id) = stem.strip_prefix("lid-mapping-") {
                let target =
                    value.as_str().map(plain_user).ok_or_else(|| Error::Store("lid mapping is not a string".into()))?;
                match id.strip_suffix("_reverse") {
                    Some(lid) => {
                        store.set(KeyKind::LidMapping, &format!("lid:{}", plain_user(lid)), target.into_bytes()).await?
                    }
                    None => {
                        store.set(KeyKind::LidMapping, &format!("pn:{}", plain_user(id)), target.into_bytes()).await?
                    }
                }
                report.lid_mappings += 1;
            } else {
                report.skipped.push(name.clone());
            }
            Ok::<_, Error>(())
        }
        .await;
        if let Err(err) = outcome {
            report.failed.push((name, err.to_string()));
        }
    }

    store.save_creds(&creds).await?;
    store.flush().await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::signal::{EncType, SignalRepository};
    use crate::store::MemoryStore;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
    }

    async fn import_and_check(auth: &Path, expected: &Value) -> (SignalRepository, String) {
        let store = Arc::new(MemoryStore::new());
        let report = import_baileys(auth, store.as_ref()).await.unwrap();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(
            (report.sessions, report.pre_keys, report.sender_keys, report.app_state_keys, report.app_state_versions),
            (1, 2, 1, 1, 1)
        );
        assert_eq!(report.lid_mappings, 2);
        assert_eq!(report.skipped.len(), 2);

        let creds = store.load_creds().await.unwrap().unwrap();
        assert_eq!(creds.me.as_ref().map(Jid::to_string).as_deref(), Some("5215511111111:7@s.whatsapp.net"));
        assert_eq!(creds.lid.as_ref().map(Jid::to_string).as_deref(), Some("111111111111111:7@lid"));
        assert_eq!((creds.registration_id, creds.push_name.as_deref()), (4321, Some("Alice Baileys")));
        assert!(creds.is_logged_in());
        let account = AdvSignedDeviceIdentity::decode(creds.account.as_deref().unwrap()).unwrap();
        assert_eq!(account.account_signature_key.map(|k| k.len()), Some(32));
        assert!(crate::crypto::verify(
            &creds.identity_key.public,
            &creds.signed_pre_key.key_pair.signal_public(),
            &creds.signed_pre_key.signature
        ));

        let key_id = expected["appStateKeyId"].as_str().unwrap();
        let stored_key = store.get(KeyKind::AppStateKey, key_id).await.unwrap().unwrap();
        assert_eq!(hex::encode(stored_key), expected["appStateKey"].as_str().unwrap());
        let lid = store.get(KeyKind::LidMapping, "pn:5215522222222").await.unwrap().unwrap();
        assert_eq!(lid, b"222222222222222");
        assert!(store.get(KeyKind::PreKey, "2").await.unwrap().is_some());

        let repo = SignalRepository::new(store.clone(), &creds);
        let bob = expected["bob"].as_str().unwrap().to_owned();
        for pending in expected["pending"].as_array().unwrap() {
            let body = hex::decode(pending["body"].as_str().unwrap()).unwrap();
            let plain = repo.decrypt(&bob, EncType::Message, &body).await.unwrap();
            assert_eq!(String::from_utf8(plain).unwrap(), pending["text"].as_str().unwrap());
        }
        let group = &expected["groupMessage"];
        let plain = repo
            .group_decrypt(
                expected["group"].as_str().unwrap(),
                expected["groupSender"].as_str().unwrap(),
                &hex::decode(group["body"].as_str().unwrap()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(String::from_utf8(plain).unwrap(), group["text"].as_str().unwrap());
        (repo, bob)
    }

    #[tokio::test]
    async fn baileys_import() {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(fixtures().join("baileys-expected.json")).unwrap()).unwrap();
        let (repo, bob) = import_and_check(&fixtures().join("baileys-auth"), &expected).await;
        let (kind, _) = repo.encrypt(&bob, b"respuesta desde soblendwa").await.unwrap();
        assert_eq!(kind, EncType::Message);
    }

    #[tokio::test]
    #[ignore = "set SOBLEND_BAILEYS_WORK to a folder produced by the libsignal export script"]
    async fn libsignal_interop() {
        let work = PathBuf::from(std::env::var("SOBLEND_BAILEYS_WORK").unwrap());
        let expected: Value = serde_json::from_slice(&std::fs::read(work.join("expected.json")).unwrap()).unwrap();
        let (repo, bob) = import_and_check(&work.join("auth"), &expected).await;
        let mut replies = Vec::new();
        for text in ["primera respuesta de soblendwa", "segunda respuesta de soblendwa"] {
            let (kind, body) = repo.encrypt(&bob, text.as_bytes()).await.unwrap();
            replies.push(serde_json::json!({ "type": kind.as_str(), "body": hex::encode(body), "text": text }));
        }
        std::fs::write(work.join("rust_replies.json"), serde_json::to_vec_pretty(&replies).unwrap()).unwrap();
    }

    #[test]
    fn baileys_buffers() {
        let expected = vec![5u8, 0, 255];
        for value in [
            serde_json::json!("BQD/"),
            serde_json::json!([5, 0, 255]),
            serde_json::json!({ "type": "Buffer", "data": "BQD/" }),
            serde_json::json!({ "type": "Buffer", "data": [5, 0, 255] }),
            serde_json::json!({ "0": 5, "1": 0, "2": 255 }),
        ] {
            assert_eq!(json_bytes(&value), Some(expected.clone()), "{value}");
        }
        assert_eq!(json_bytes(&serde_json::json!({ "1": 5 })), None);
        assert_eq!(plain_user("123456:7@lid"), "123456");
    }
}
