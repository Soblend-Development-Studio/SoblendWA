use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::bson::spec::BinarySubtype;
use mongodb::bson::{doc, Binary, Bson, Document};
use mongodb::options::IndexOptions;
use mongodb::{Client, Collection, Database, IndexModel};

use super::batched::{Backend, BatchedStore, KeyWrite};
use super::{Creds, KeyKind};
use crate::error::{Error, Result};

const KEYS: &str = "soblend_keys";
const CREDS: &str = "soblend_creds";
const MAX_BATCH_BYTES: usize = 8 * 1024 * 1024;
const MAX_BATCH_WRITES: usize = 1000;

fn failed(err: impl std::fmt::Display) -> Error {
    Error::Store(format!("mongodb: {err}"))
}

pub(crate) fn binary(bytes: &[u8]) -> Bson {
    Bson::Binary(Binary { subtype: BinarySubtype::Generic, bytes: bytes.to_vec() })
}

fn key_id(account: &str, kind: KeyKind, key: &str) -> String {
    format!("{account}:{}:{key}", kind.code())
}

fn key_document(account: &str, kind: KeyKind, key: &str, value: &[u8]) -> Document {
    doc! {
        "q": { "_id": key_id(account, kind, key) },
        "u": { "$set": {
            "account": account,
            "kind": i32::from(kind.code()),
            "key": key,
            "value": binary(value),
        } },
        "upsert": true,
    }
}

fn batches(writes: &[KeyWrite], account: &str) -> (bool, Vec<Vec<Document>>, Vec<String>) {
    let mut cleared = false;
    let mut upserts = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0;
    let mut removes = Vec::new();
    for write in writes {
        match write {
            KeyWrite::Clear => cleared = true,
            KeyWrite::Remove(kind, key) => removes.push(key_id(account, *kind, key)),
            KeyWrite::Set(kind, key, value) => {
                if current.len() >= MAX_BATCH_WRITES || current_bytes + value.len() > MAX_BATCH_BYTES {
                    upserts.push(std::mem::take(&mut current));
                    current_bytes = 0;
                }
                current_bytes += value.len() + key.len();
                current.push(key_document(account, *kind, key, value));
            }
        }
    }
    if !current.is_empty() {
        upserts.push(current);
    }
    (cleared, upserts, removes)
}

fn check_write(reply: &Document) -> Result<()> {
    match reply.get_array("writeErrors") {
        Ok(errors) if !errors.is_empty() => {
            Err(failed(format!("{} writes were refused: {:?}", errors.len(), errors[0])))
        }
        _ => Ok(()),
    }
}

pub struct Mongo {
    database: Database,
    keys: Collection<Document>,
    creds: Collection<Document>,
    account: String,
}

pub type MongoStore = BatchedStore<Mongo>;

impl BatchedStore<Mongo> {
    pub async fn connect(url: &str, database: &str) -> Result<Self> {
        Self::connect_account(url, database, "default").await
    }

    pub async fn connect_account(url: &str, database: &str, account: &str) -> Result<Self> {
        let client = Client::with_uri_str(url).await.map_err(failed)?;
        Self::from_database(client.database(database), account).await
    }

    pub async fn from_database(database: Database, account: &str) -> Result<Self> {
        database.run_command(doc! { "ping": 1 }).await.map_err(failed)?;
        let keys = database.collection::<Document>(KEYS);
        let index = IndexModel::builder()
            .keys(doc! { "account": 1, "kind": 1 })
            .options(IndexOptions::builder().name("soblend_keys_by_account_and_kind".to_owned()).build())
            .build();
        keys.create_index(index).await.map_err(failed)?;
        let creds = database.collection::<Document>(CREDS);
        Ok(BatchedStore::new(Mongo { database, keys, creds, account: account.to_owned() }))
    }
}

#[async_trait]
impl Backend for Mongo {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        let found = self.creds.find_one(doc! { "_id": &self.account }).await.map_err(failed)?;
        let Some(document) = found else { return Ok(None) };
        let json = document.get_str("data").map_err(failed)?;
        serde_json::from_str(json).map(Some).map_err(Error::from)
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        let json = serde_json::to_string(creds)?;
        self.creds
            .update_one(doc! { "_id": &self.account }, doc! { "$set": { "data": json } })
            .upsert(true)
            .await
            .map(drop)
            .map_err(failed)
    }

    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        let found = self.keys.find_one(doc! { "_id": key_id(&self.account, kind, key) }).await.map_err(failed)?;
        Ok(found.and_then(|document| match document.get("value") {
            Some(Bson::Binary(value)) => Some(value.bytes.clone()),
            _ => None,
        }))
    }

    async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
        let cursor = self
            .keys
            .find(doc! { "account": &self.account, "kind": i32::from(kind.code()) })
            .projection(doc! { "key": 1 })
            .await
            .map_err(failed)?;
        let documents: Vec<Document> = cursor.try_collect().await.map_err(failed)?;
        Ok(documents.iter().filter_map(|document| document.get_str("key").ok().map(str::to_owned)).collect())
    }

    async fn apply(&self, writes: &[KeyWrite]) -> Result<()> {
        let (cleared, upserts, removes) = batches(writes, &self.account);
        if cleared {
            self.keys.delete_many(doc! { "account": &self.account }).await.map_err(failed)?;
            self.creds.delete_one(doc! { "_id": &self.account }).await.map_err(failed)?;
        }
        for chunk in removes.chunks(MAX_BATCH_WRITES) {
            self.keys.delete_many(doc! { "_id": { "$in": chunk } }).await.map_err(failed)?;
        }
        for updates in upserts {
            let reply = self
                .database
                .run_command(doc! { "update": KEYS, "updates": updates, "ordered": false })
                .await
                .map_err(failed)?;
            check_write(&reply)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{batched, Store};

    #[test]
    fn writes_are_split_into_safe_commands() {
        let big = vec![7u8; 3 * 1024 * 1024];
        let mut writes: Vec<KeyWrite> =
            (0..5).map(|i| KeyWrite::Set(KeyKind::Session, format!("big{i}"), big.clone())).collect();
        writes.extend((0..2500).map(|i| KeyWrite::Set(KeyKind::PreKey, i.to_string(), vec![1; 32])));
        writes.push(KeyWrite::Remove(KeyKind::SenderKey, "viejo".into()));
        let (cleared, upserts, removes) = batches(&writes, "bot");
        assert!(!cleared);
        assert_eq!(removes, vec![format!("bot:{}:viejo", KeyKind::SenderKey.code())]);
        assert_eq!(upserts.iter().map(Vec::len).sum::<usize>(), 2505);
        assert!(upserts.iter().all(|chunk| chunk.len() <= MAX_BATCH_WRITES));
        for chunk in &upserts {
            let bytes: usize = chunk
                .iter()
                .map(|update| match update.get_document("u").unwrap().get_document("$set").unwrap().get("value") {
                    Some(Bson::Binary(value)) => value.bytes.len(),
                    _ => 0,
                })
                .sum();
            assert!(bytes <= MAX_BATCH_BYTES, "{bytes}");
        }
        assert!(batches(&[KeyWrite::Clear], "bot").0);
    }

    #[tokio::test]
    #[ignore = "set SOBLEND_MONGO_URL to a MongoDB you can write to"]
    async fn mongo_round_trip_and_account_isolation() {
        let url = std::env::var("SOBLEND_MONGO_URL").unwrap();
        let database = format!("soblendwa_test_{}", crate::crypto::random_u32());
        let store = MongoStore::connect_account(&url, &database, "bot").await.unwrap();
        batched::tests::exercise(&store).await;

        let other = MongoStore::connect_account(&url, &database, "otro").await.unwrap();
        store.set(KeyKind::Session, "5215500000000.0", vec![7; 64]).await.unwrap();
        other.set(KeyKind::Session, "5215500000000.0", vec![9]).await.unwrap();
        store.save_creds(&Creds::generate()).await.unwrap();
        store.flush().await.unwrap();
        other.flush().await.unwrap();

        let reopened = MongoStore::connect_account(&url, &database, "bot").await.unwrap();
        assert!(reopened.load_creds().await.unwrap().is_some());
        assert_eq!(reopened.get(KeyKind::Session, "5215500000000.0").await.unwrap(), Some(vec![7; 64]));
        assert_eq!(reopened.keys(KeyKind::Session).await.unwrap(), vec!["5215500000000.0".to_owned()]);
        let isolated = MongoStore::connect_account(&url, &database, "otro").await.unwrap();
        assert!(isolated.load_creds().await.unwrap().is_none());
        assert_eq!(isolated.get(KeyKind::Session, "5215500000000.0").await.unwrap(), Some(vec![9]));
        store.backend().database.drop().await.unwrap();
    }
}
