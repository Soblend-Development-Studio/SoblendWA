use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use soblendwa::store::{
    export_zapo_mongo, export_zapo_sqlite, import_zapo_mongo, import_zapo_sqlite, zapo_sessions, ZapoExport,
    ZapoImport, ZapoMongo,
};
use soblendwa::{Error, MemoryStore, Result, Store};

use crate::{open_store, StoreOptions};

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ZapoSource {
    Sqlite {
        path: String,
        session: Option<String>,
    },
    Mongo {
        url: String,
        database: String,
        session: Option<String>,
        #[serde(rename = "collectionPrefix", alias = "prefix")]
        collection_prefix: Option<String>,
    },
}

fn pick(sessions: Vec<String>, given: Option<String>, place: &str) -> Result<String> {
    if let Some(session) = given {
        return Ok(session);
    }
    match sessions.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(Error::InvalidArgument(format!("{place} has no zapo session"))),
        many => {
            Err(Error::InvalidArgument(format!("{place} holds several zapo sessions, name one: {}", many.join(", "))))
        }
    }
}

async fn mongo(url: &str, database: &str, prefix: &Option<String>) -> Result<ZapoMongo> {
    let source = ZapoMongo::connect(url, database).await?;
    Ok(match prefix {
        Some(prefix) => source.collection_prefix(prefix.clone()),
        None => source,
    })
}

fn failures(failed: &[(String, String)]) -> Value {
    Value::Array(failed.iter().map(|(item, error)| json!({ "item": item, "error": error })).collect())
}

fn import_report(report: &ZapoImport, session: &str, checked: bool) -> Value {
    json!({
        "session": session,
        "checkOnly": checked,
        "me": report.me.as_ref().map(|j| j.to_string()),
        "sessions": report.sessions,
        "preKeys": report.pre_keys,
        "senderKeys": report.sender_keys,
        "appStateKeys": report.app_state_keys,
        "appStateVersions": report.app_state_versions,
        "lidMappings": report.lid_mappings,
        "failed": failures(&report.failed),
    })
}

fn export_report(report: &ZapoExport, session: &str) -> Value {
    json!({
        "session": session,
        "sessions": report.sessions,
        "preKeys": report.pre_keys,
        "senderKeys": report.sender_keys,
        "appStateKeys": report.app_state_keys,
        "appStateVersions": report.app_state_versions,
        "failed": failures(&report.failed),
    })
}

pub async fn import_zapo(source: ZapoSource, target: Option<StoreOptions>, check: bool) -> Result<Value> {
    let store: Arc<dyn Store> = match (&target, check) {
        (_, true) => Arc::new(MemoryStore::new()),
        (Some(target), false) => open_store(target).await?,
        (None, false) => {
            return Err(Error::InvalidArgument("say where to store the session, or pass check: true".into()));
        }
    };
    if !check && store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
        return Err(Error::InvalidArgument("the target store already holds a linked session; use an empty one".into()));
    }
    let (report, session) = match source {
        ZapoSource::Sqlite { path, session } => {
            let session = pick(zapo_sessions(&path)?, session, &path)?;
            (import_zapo_sqlite(&path, &session, store.as_ref()).await?, session)
        }
        ZapoSource::Mongo { url, database, session, collection_prefix } => {
            let source = mongo(&url, &database, &collection_prefix).await?;
            let session = pick(source.sessions().await?, session, &database)?;
            (import_zapo_mongo(&source, &session, store.as_ref()).await?, session)
        }
    };
    if !check {
        store.flush().await?;
    }
    Ok(import_report(&report, &session, check))
}

pub async fn rollback_zapo(source: StoreOptions, target: ZapoSource) -> Result<Value> {
    let store = open_store(&source).await?;
    if !store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
        return Err(Error::InvalidArgument("the SoblendWA store holds no linked session to hand back".into()));
    }
    match target {
        ZapoSource::Sqlite { path, session } => {
            let session = pick(zapo_sessions(&path)?, session, &path)?;
            Ok(export_report(&export_zapo_sqlite(store.as_ref(), &path, &session).await?, &session))
        }
        ZapoSource::Mongo { url, database, session, collection_prefix } => {
            let target = mongo(&url, &database, &collection_prefix).await?;
            let session = pick(target.sessions().await?, session, &database)?;
            Ok(export_report(&export_zapo_mongo(store.as_ref(), &target, &session).await?, &session))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        format!("{}/../../tests/fixtures/zapo/alice.sqlite", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn check_only() {
        let source = ZapoSource::Sqlite { path: fixture(), session: None };
        let before = std::fs::read(fixture()).unwrap();
        let report = crate::RUNTIME.block_on(import_zapo(source, None, true)).unwrap();
        assert_eq!(report["checkOnly"], true);
        assert!(report["sessions"].as_u64().unwrap() > 0, "{report}");
        assert_eq!(report["failed"], json!([]));
        assert_eq!(std::fs::read(fixture()).unwrap(), before, "the zapo database must stay untouched");
    }

    #[test]
    fn import_refuses_linked_store() {
        let dir = std::env::temp_dir().join(format!("soblendwa-zapo-bridge-{}", soblendwa::crypto::random_u32()));
        let target = StoreOptions::File { path: dir.to_string_lossy().into_owned() };
        let source = || ZapoSource::Sqlite { path: fixture(), session: None };
        let first = crate::RUNTIME.block_on(import_zapo(source(), Some(target.clone()), false)).unwrap();
        assert_eq!(first["checkOnly"], false);
        let again = crate::RUNTIME.block_on(import_zapo(source(), Some(target), false)).unwrap_err();
        assert_eq!(again.code(), "INVALID_ARGUMENT");
        let _ = std::fs::remove_dir_all(dir);
    }
}
