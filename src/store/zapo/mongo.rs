use futures_util::TryStreamExt;
use mongodb::bson::{doc, Bson, Document};
use mongodb::{Client, Collection, Database};

use super::{apply, collect, failed, key_epoch, Outgoing, ZapoData, ZapoExport, ZapoImport};
use crate::error::Result;
use crate::jid::Jid;
use crate::store::mongo::binary;
use crate::store::{Creds, Store};

const INSERT_CHUNK: usize = 1000;

#[derive(Clone)]
pub struct ZapoMongo {
    database: Database,
    prefix: String,
}

#[derive(Default)]
struct Documents {
    auth: Option<Document>,
    registration: Option<Document>,
    signed: Option<Document>,
    meta: Option<Document>,
    sessions: Vec<Document>,
    pre_keys: Vec<Document>,
    sender_keys: Vec<Document>,
    app_state_keys: Vec<Document>,
    versions: Vec<Document>,
    index_values: Vec<Document>,
    contacts: Vec<Document>,
    device_lists: Vec<Document>,
}

fn bytes(document: &Document, key: &str) -> Option<Vec<u8>> {
    match document.get(key)? {
        Bson::Binary(value) => Some(value.bytes.clone()),
        _ => None,
    }
}

fn number(document: &Document, key: &str) -> Option<i64> {
    match document.get(key)? {
        Bson::Int32(value) => Some(i64::from(*value)),
        Bson::Int64(value) => Some(*value),
        Bson::Double(value) => Some(*value as i64),
        _ => None,
    }
}

fn text(document: &Document, key: &str) -> Option<String> {
    document.get_str(key).ok().map(str::to_owned)
}

fn required_bytes(document: &Document, key: &str) -> Result<Vec<u8>> {
    bytes(document, key).ok_or_else(|| failed(format!("{key} is missing or is not binary")))
}

fn required_number(document: &Document, key: &str) -> Result<i64> {
    number(document, key).ok_or_else(|| failed(format!("{key} is missing or is not a number")))
}

fn parse(documents: Documents, session: &str) -> Result<ZapoData> {
    let auth = documents.auth.ok_or_else(|| failed(format!("there are no credentials for the session {session:?}")))?;
    let mut data = ZapoData {
        noise: (required_bytes(&auth, "noise_pub_key")?, required_bytes(&auth, "noise_priv_key")?),
        registration: (
            required_number(&auth, "registration_id")? as u32,
            required_bytes(&auth, "identity_pub_key")?,
            required_bytes(&auth, "identity_priv_key")?,
        ),
        signed: (
            required_number(&auth, "signed_prekey_id")? as u32,
            required_bytes(&auth, "signed_prekey_pub_key")?,
            required_bytes(&auth, "signed_prekey_priv_key")?,
            required_bytes(&auth, "signed_prekey_signature")?,
        ),
        adv_secret: required_bytes(&auth, "adv_secret_key")?,
        account: bytes(&auth, "signed_identity"),
        me: text(&auth, "me_jid"),
        lid: text(&auth, "me_lid"),
        name: text(&auth, "me_display_name"),
        platform: text(&auth, "platform"),
        routing_info: bytes(&auth, "routing_info"),
        ..Default::default()
    };
    if let Some(row) = &documents.registration {
        data.registration = (
            required_number(row, "registration_id")? as u32,
            required_bytes(row, "identity_pub_key")?,
            required_bytes(row, "identity_priv_key")?,
        );
    }
    if let Some(row) = &documents.signed {
        data.signed = (
            required_number(row, "key_id")? as u32,
            required_bytes(row, "pub_key")?,
            required_bytes(row, "priv_key")?,
            required_bytes(row, "signature")?,
        );
    }
    data.next_pre_key = documents.meta.as_ref().and_then(|meta| number(meta, "next_prekey_id")).map(|next| next as u32);

    let identified = |document: &Document| document.get_document("_id").ok().cloned().unwrap_or_default();
    data.sessions = documents
        .sessions
        .iter()
        .filter_map(|row| {
            let id = identified(row);
            Some((text(&id, "user")?, text(&id, "server")?, number(&id, "device")? as u16, bytes(row, "record")?))
        })
        .collect();
    data.pre_keys = documents
        .pre_keys
        .iter()
        .filter_map(|row| Some((number(&identified(row), "key_id")? as u32, bytes(row, "priv_key")?)))
        .collect();
    data.sender_keys = documents
        .sender_keys
        .iter()
        .filter_map(|row| {
            let id = identified(row);
            Some((
                text(&id, "group_id")?,
                text(&id, "sender_user")?,
                text(&id, "sender_server")?,
                number(&id, "sender_device")? as u16,
                bytes(row, "record")?,
            ))
        })
        .collect();
    data.app_state_keys = documents
        .app_state_keys
        .iter()
        .filter_map(|row| Some((bytes(&identified(row), "key_id")?, bytes(row, "key_data")?)))
        .collect();
    data.versions = documents
        .versions
        .iter()
        .filter_map(|row| {
            Some((text(&identified(row), "collection")?, number(row, "version")?.max(0) as u64, bytes(row, "hash")?))
        })
        .collect();
    data.index_values = documents
        .index_values
        .iter()
        .filter_map(|row| {
            let id = identified(row);
            Some((text(&id, "collection")?, text(&id, "index_mac_hex")?, bytes(row, "value_mac")?))
        })
        .collect();
    data.lid_pairs.extend(documents.contacts.iter().map(|row| (text(row, "phone_number"), text(row, "lid"))));
    data.lid_pairs.extend(
        documents.device_lists.iter().map(|row| (text(&identified(row), "user_jid"), text(row, "alt_user_jid"))),
    );
    Ok(data)
}

impl ZapoMongo {
    pub async fn connect(url: &str, database: &str) -> Result<Self> {
        let client = Client::with_uri_str(url).await.map_err(failed)?;
        Ok(Self::new(client.database(database)))
    }

    pub fn new(database: Database) -> Self {
        ZapoMongo { database, prefix: String::new() }
    }

    pub fn collection_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    fn col(&self, name: &str) -> Collection<Document> {
        self.database.collection(&format!("{}{name}", self.prefix))
    }

    async fn one(&self, name: &str, session: &str) -> Result<Option<Document>> {
        self.col(name).find_one(doc! { "_id": session }).await.map_err(failed)
    }

    async fn many(&self, name: &str, session: &str) -> Result<Vec<Document>> {
        let cursor = self.col(name).find(doc! { "_id.session_id": session }).await.map_err(failed)?;
        cursor.try_collect().await.map_err(failed)
    }

    async fn documents(&self, session: &str) -> Result<Documents> {
        Ok(Documents {
            auth: self.one("auth_credentials", session).await?,
            registration: self.one("signal_registration", session).await?,
            signed: self.one("signal_signed_prekey", session).await?,
            meta: self.one("signal_meta", session).await?,
            sessions: self.many("signal_sessions", session).await?,
            pre_keys: self.many("signal_prekeys", session).await?,
            sender_keys: self.many("sender_keys", session).await?,
            app_state_keys: self.many("appstate_sync_keys", session).await?,
            versions: self.many("appstate_collection_versions", session).await?,
            index_values: self.many("appstate_collection_index_values", session).await?,
            contacts: self.many("mailbox_contacts", session).await?,
            device_lists: self.many("device_list_cache", session).await?,
        })
    }

    pub async fn sessions(&self) -> Result<Vec<String>> {
        let cursor = self.col("auth_credentials").find(doc! {}).projection(doc! { "_id": 1 }).await.map_err(failed)?;
        let documents: Vec<Document> = cursor.try_collect().await.map_err(failed)?;
        let mut names: Vec<String> = documents.iter().filter_map(|row| text(row, "_id")).collect();
        names.sort();
        Ok(names)
    }

    async fn replace(&self, name: &str, session: &str, rows: Vec<Document>) -> Result<()> {
        let collection = self.col(name);
        collection.delete_many(doc! { "_id.session_id": session }).await.map_err(failed)?;
        for chunk in rows.chunks(INSERT_CHUNK) {
            collection.insert_many(chunk).await.map_err(failed)?;
        }
        Ok(())
    }

    async fn write_back(&self, session: &str, creds: &Creds, out: &Outgoing, report: &mut ZapoExport) -> Result<()> {
        if self.one("auth_credentials", session).await?.is_none() {
            return Err(failed(format!("the zapo database has no session {session:?} to roll back into")));
        }
        let identity_public = binary(&creds.identity_key.public);
        let identity_private = binary(&creds.identity_key.private);
        let signed = &creds.signed_pre_key;
        let mut auth = doc! {
            "registration_id": creds.registration_id as i32,
            "identity_pub_key": identity_public.clone(),
            "identity_priv_key": identity_private.clone(),
            "signed_prekey_id": signed.id as i32,
            "signed_prekey_pub_key": binary(&signed.key_pair.public),
            "signed_prekey_priv_key": binary(&signed.key_pair.private),
            "signed_prekey_signature": binary(&signed.signature),
        };
        if let Some(account) = &creds.account {
            auth.insert("signed_identity", binary(account));
        }
        if let Some(me) = &creds.me {
            auth.insert("me_jid", Jid::to_string(me));
        }
        if let Some(lid) = &creds.lid {
            auth.insert("me_lid", Jid::to_string(lid));
        }
        if let Some(name) = &creds.push_name {
            auth.insert("me_display_name", name.clone());
        }
        if let Some(routing) = &creds.routing_info {
            auth.insert("routing_info", binary(routing));
        }
        let by_session = doc! { "_id": session };
        self.col("auth_credentials").update_one(by_session.clone(), doc! { "$set": auth }).await.map_err(failed)?;
        self.col("signal_registration")
            .update_one(
                by_session.clone(),
                doc! { "$set": {
                    "registration_id": creds.registration_id as i32,
                    "identity_pub_key": identity_public,
                    "identity_priv_key": identity_private,
                } },
            )
            .upsert(true)
            .await
            .map_err(failed)?;
        self.col("signal_signed_prekey")
            .update_one(
                by_session.clone(),
                doc! { "$set": {
                    "key_id": signed.id as i32,
                    "pub_key": binary(&signed.key_pair.public),
                    "priv_key": binary(&signed.key_pair.private),
                    "signature": binary(&signed.signature),
                    "uploaded": true,
                } },
            )
            .upsert(true)
            .await
            .map_err(failed)?;
        self.col("signal_meta")
            .update_one(
                by_session,
                doc! {
                    "$set": { "next_prekey_id": creds.next_pre_key_id as i32 },
                    "$setOnInsert": { "server_has_prekeys": true },
                },
            )
            .upsert(true)
            .await
            .map_err(failed)?;

        let (sessions, identities, pre_keys, sender_keys, versions, index_values) = rows_for(session, out);
        self.replace("signal_sessions", session, sessions).await?;
        self.replace("signal_identities", session, identities).await?;
        self.replace("signal_prekeys", session, pre_keys).await?;
        self.replace("sender_keys", session, sender_keys).await?;
        self.replace("sender_key_distribution", session, Vec::new()).await?;
        self.replace("appstate_collection_versions", session, versions).await?;
        self.replace("appstate_collection_index_values", session, index_values).await?;
        report.sessions = out.sessions.len();
        report.pre_keys = out.pre_keys.len();
        report.sender_keys = out.sender_keys.len();
        report.app_state_versions = out.versions.len();

        let now_ms = crate::message::now_secs() * 1000;
        let keys = self.col("appstate_sync_keys");
        for (id, data, newest) in &out.app_state_keys {
            keys.update_one(
                doc! { "_id": { "session_id": session, "key_id": binary(id) } },
                doc! {
                    "$set": { "key_data": binary(data), "key_epoch": key_epoch(id) },
                    "$max": { "timestamp": if *newest { now_ms } else { 0 } },
                },
            )
            .upsert(true)
            .await
            .map_err(failed)?;
            report.app_state_keys += 1;
        }
        Ok(())
    }
}

type ZapoRows = (Vec<Document>, Vec<Document>, Vec<Document>, Vec<Document>, Vec<Document>, Vec<Document>);

fn rows_for(session: &str, out: &Outgoing) -> ZapoRows {
    let address = |user: &str, server: &str, device: u16| {
        doc! { "session_id": session, "user": user, "server": server, "device": i32::from(device) }
    };
    let sessions = out
        .sessions
        .iter()
        .map(|(user, server, device, record, _)| doc! { "_id": address(user, server, *device), "record": binary(record) })
        .collect();
    let identities = out
        .sessions
        .iter()
        .filter_map(|(user, server, device, _, remote)| {
            Some(doc! { "_id": address(user, server, *device), "identity_key": binary(remote.as_ref()?) })
        })
        .collect();
    let pre_keys = out
        .pre_keys
        .iter()
        .map(|(id, public, private, uploaded)| {
            doc! {
                "_id": { "session_id": session, "key_id": *id as i32 },
                "pub_key": binary(public),
                "priv_key": binary(private),
                "uploaded": *uploaded,
            }
        })
        .collect();
    let sender_keys = out
        .sender_keys
        .iter()
        .map(|(group, user, server, device, record)| {
            doc! {
                "_id": {
                    "session_id": session,
                    "group_id": group,
                    "sender_user": user,
                    "sender_server": server,
                    "sender_device": i32::from(*device),
                },
                "record": binary(record),
            }
        })
        .collect();
    let versions = out
        .versions
        .iter()
        .map(|(name, state)| {
            doc! {
                "_id": { "session_id": session, "collection": name },
                "version": state.version as i64,
                "hash": binary(&state.hash),
            }
        })
        .collect();
    let index_values = out
        .versions
        .iter()
        .flat_map(|(name, state)| {
            state.entries.iter().map(move |entry| {
                doc! {
                    "_id": {
                        "session_id": session,
                        "collection": name,
                        "index_mac_hex": hex::encode(&entry.index_mac),
                    },
                    "value_mac": binary(&entry.value_mac),
                }
            })
        })
        .collect();
    (sessions, identities, pre_keys, sender_keys, versions, index_values)
}

pub async fn import_zapo_mongo(source: &ZapoMongo, session: &str, store: &dyn Store) -> Result<ZapoImport> {
    let snapshot = parse(source.documents(session).await?, session)?.convert()?;
    apply(snapshot, store).await
}

pub async fn export_zapo_mongo(store: &dyn Store, target: &ZapoMongo, session: &str) -> Result<ZapoExport> {
    store.flush().await?;
    let creds = store.load_creds().await?.ok_or_else(|| failed("the store has no credentials to roll back"))?;
    let mut report = ZapoExport::default();
    let out = collect(store, &creds, &mut report).await?;
    target.write_back(session, &creds, &out, &mut report).await?;
    Ok(report)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rusqlite::{Connection, OpenFlags};
    use serde_json::Value;

    use super::*;
    use crate::signal::{EncType, SignalRepository};
    use crate::store::{KeyKind, MemoryStore};

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("zapo")
    }

    fn blob(row: &rusqlite::Row<'_>, column: &str) -> Bson {
        binary(&row.get::<_, Vec<u8>>(column).unwrap())
    }

    fn all(db: &Connection, sql: &str, map: impl Fn(&rusqlite::Row<'_>) -> Document) -> Vec<Document> {
        let mut statement = db.prepare(sql).unwrap();
        let rows = statement.query_map([], |row| Ok(map(row))).unwrap();
        rows.map(Result::unwrap).collect()
    }

    fn documents_like_zapo_writes_them() -> Documents {
        let path = fixtures().join("alice.sqlite");
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let one =
            |sql: &str, map: &dyn Fn(&rusqlite::Row<'_>) -> Document| db.query_row(sql, [], |row| Ok(map(row))).ok();
        Documents {
            auth: one("SELECT * FROM auth_credentials", &|row| {
                doc! {
                    "_id": "bot",
                    "noise_pub_key": blob(row, "noise_pub_key"),
                    "noise_priv_key": blob(row, "noise_priv_key"),
                    "registration_id": row.get::<_, i32>("registration_id").unwrap(),
                    "identity_pub_key": blob(row, "identity_pub_key"),
                    "identity_priv_key": blob(row, "identity_priv_key"),
                    "signed_prekey_id": row.get::<_, i32>("signed_prekey_id").unwrap(),
                    "signed_prekey_pub_key": blob(row, "signed_prekey_pub_key"),
                    "signed_prekey_priv_key": blob(row, "signed_prekey_priv_key"),
                    "signed_prekey_signature": blob(row, "signed_prekey_signature"),
                    "adv_secret_key": blob(row, "adv_secret_key"),
                    "signed_identity": blob(row, "signed_identity"),
                    "me_jid": row.get::<_, String>("me_jid").unwrap(),
                    "me_lid": row.get::<_, String>("me_lid").unwrap(),
                    "me_display_name": row.get::<_, String>("me_display_name").unwrap(),
                    "platform": row.get::<_, String>("platform").unwrap(),
                    "routing_info": Bson::Null,
                }
            }),
            meta: one("SELECT next_prekey_id FROM signal_meta", &|row| {
                doc! { "_id": "bot", "next_prekey_id": row.get::<_, i32>(0).unwrap() }
            }),
            sessions: all(&db, "SELECT * FROM signal_session", |row| {
                doc! {
                    "_id": {
                        "session_id": "bot",
                        "user": row.get::<_, String>("user").unwrap(),
                        "server": row.get::<_, String>("server").unwrap(),
                        "device": row.get::<_, i32>("device").unwrap(),
                    },
                    "record": blob(row, "record"),
                }
            }),
            pre_keys: all(&db, "SELECT * FROM signal_prekey", |row| {
                doc! {
                    "_id": { "session_id": "bot", "key_id": row.get::<_, i32>("key_id").unwrap() },
                    "pub_key": blob(row, "pub_key"),
                    "priv_key": blob(row, "priv_key"),
                    "uploaded": false,
                }
            }),
            sender_keys: all(&db, "SELECT * FROM sender_keys", |row| {
                doc! {
                    "_id": {
                        "session_id": "bot",
                        "group_id": row.get::<_, String>("group_id").unwrap(),
                        "sender_user": row.get::<_, String>("sender_user").unwrap(),
                        "sender_server": row.get::<_, String>("sender_server").unwrap(),
                        "sender_device": row.get::<_, i32>("sender_device").unwrap(),
                    },
                    "record": blob(row, "record"),
                }
            }),
            app_state_keys: all(&db, "SELECT * FROM appstate_sync_keys", |row| {
                doc! {
                    "_id": { "session_id": "bot", "key_id": blob(row, "key_id") },
                    "key_data": blob(row, "key_data"),
                    "timestamp": row.get::<_, i64>("timestamp").unwrap(),
                }
            }),
            versions: all(&db, "SELECT * FROM appstate_collection_versions", |row| {
                doc! {
                    "_id": { "session_id": "bot", "collection": row.get::<_, String>("collection").unwrap() },
                    "version": row.get::<_, i32>("version").unwrap(),
                    "hash": blob(row, "hash"),
                }
            }),
            index_values: all(&db, "SELECT * FROM appstate_collection_index_values", |row| {
                doc! {
                    "_id": {
                        "session_id": "bot",
                        "collection": row.get::<_, String>("collection").unwrap(),
                        "index_mac_hex": row.get::<_, String>("index_mac_hex").unwrap(),
                    },
                    "value_mac": blob(row, "value_mac"),
                }
            }),
            contacts: all(&db, "SELECT * FROM mailbox_contacts", |row| {
                doc! {
                    "_id": { "session_id": "bot", "jid": row.get::<_, String>("jid").unwrap() },
                    "lid": row.get::<_, Option<String>>("lid").unwrap(),
                    "phone_number": row.get::<_, Option<String>>("phone_number").unwrap(),
                }
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn a_mongo_shaped_zapo_session_decrypts_its_pending_messages() {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(fixtures().join("expected.json")).unwrap()).unwrap();
        let snapshot = parse(documents_like_zapo_writes_them(), "bot").unwrap().convert().unwrap();
        let store = Arc::new(MemoryStore::new());
        let report = apply(snapshot, store.as_ref()).await.unwrap();
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
        let creds = store.load_creds().await.unwrap().unwrap();
        assert_eq!(creds.me.as_ref().map(Jid::to_string).as_deref(), expected["me"].as_str());

        let repo = SignalRepository::new(store.clone(), &creds);
        for pending in expected["pending"].as_array().unwrap() {
            let from = expected[pending["from"].as_str().unwrap()].as_str().unwrap();
            let kind = if pending["type"] == "pkmsg" { EncType::PreKey } else { EncType::Message };
            let body = hex::decode(pending["body"].as_str().unwrap()).unwrap();
            let plain = repo.decrypt(from, kind, &body).await.unwrap();
            assert_eq!(String::from_utf8(plain).unwrap(), pending["text"].as_str().unwrap());
        }

        let mut report = ZapoExport::default();
        let out = collect(store.as_ref(), &creds, &mut report).await.unwrap();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        let (sessions, identities, pre_keys, sender_keys, versions, index_values) = rows_for("bot", &out);
        let round_trip = Documents {
            auth: documents_like_zapo_writes_them().auth,
            sessions,
            pre_keys,
            sender_keys,
            versions,
            index_values,
            ..Default::default()
        };
        assert_eq!(identities.len(), 2);
        assert_eq!(identities[0].get_document("_id").unwrap().get_str("session_id").unwrap(), "bot");
        let again = parse(round_trip, "bot").unwrap().convert().unwrap();
        assert!(again.report.failed.is_empty(), "{:?}", again.report.failed);
        assert_eq!((again.report.sessions, again.report.sender_keys), (2, 2));
        assert!(store
            .get(
                KeyKind::SenderKey,
                &format!("{}::{}", expected["group"].as_str().unwrap(), expected["groupSender"].as_str().unwrap())
            )
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    #[ignore = "set SOBLEND_MONGO_URL to a MongoDB you can write to"]
    async fn imports_and_rolls_back_a_session_in_a_real_mongodb() {
        let url = std::env::var("SOBLEND_MONGO_URL").unwrap();
        let name = format!("soblendwa_zapo_test_{}", crate::crypto::random_u32());
        let client = Client::with_uri_str(&url).await.unwrap();
        let database = client.database(&name);
        let source = documents_like_zapo_writes_them();
        let target = ZapoMongo::new(database.clone());
        target.col("auth_credentials").insert_one(source.auth.clone().unwrap()).await.unwrap();
        target.col("signal_sessions").insert_many(&source.sessions).await.unwrap();
        target.col("sender_keys").insert_many(&source.sender_keys).await.unwrap();
        target.col("signal_prekeys").insert_many(&source.pre_keys).await.unwrap();
        assert_eq!(target.sessions().await.unwrap(), vec!["bot".to_owned()]);

        let store = MemoryStore::new();
        let report = import_zapo_mongo(&target, "bot", &store).await.unwrap();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!((report.sessions, report.sender_keys), (1, 2));
        let back = export_zapo_mongo(&store, &target, "bot").await.unwrap();
        assert!(back.failed.is_empty(), "{:?}", back.failed);
        let again = MemoryStore::new();
        let report = import_zapo_mongo(&target, "bot", &again).await.unwrap();
        assert_eq!((report.sessions, report.sender_keys), (1, 2));
        database.drop().await.unwrap();
    }
}
