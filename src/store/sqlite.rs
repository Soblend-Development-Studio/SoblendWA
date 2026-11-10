use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use super::batched::{Backend, BatchedStore, KeyWrite};
use super::{Creds, KeyKind};
use crate::error::{Error, Result};

const SCHEMA: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA busy_timeout = 5000;
    CREATE TABLE IF NOT EXISTS soblend_creds (
        account TEXT PRIMARY KEY,
        data TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS soblend_keys (
        account TEXT NOT NULL,
        kind INTEGER NOT NULL,
        key TEXT NOT NULL,
        value BLOB NOT NULL,
        PRIMARY KEY (account, kind, key)
    ) WITHOUT ROWID;
";

const UPSERT: &str = "INSERT INTO soblend_keys (account, kind, key, value) VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (account, kind, key) DO UPDATE SET value = excluded.value";

fn failed(err: impl std::fmt::Display) -> Error {
    Error::Store(format!("sqlite: {err}"))
}

pub struct Sqlite {
    connection: Arc<Mutex<Connection>>,
    account: Arc<str>,
}

pub type SqliteStore = BatchedStore<Sqlite>;

impl BatchedStore<Sqlite> {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_account(path, "default").await
    }

    pub async fn open_account(path: impl AsRef<Path>, account: &str) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let connection = tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            Connection::open(&path).map_err(failed)
        })
        .await
        .map_err(failed)??;
        Self::from_connection(connection, account)
    }

    pub fn in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory().map_err(failed)?, "default")
    }

    pub fn from_connection(connection: Connection, account: &str) -> Result<Self> {
        connection.execute_batch(SCHEMA).map_err(failed)?;
        Ok(BatchedStore::new(Sqlite { connection: Arc::new(Mutex::new(connection)), account: account.into() }))
    }
}

impl Sqlite {
    async fn run<T, F>(&self, job: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection, &str) -> rusqlite::Result<T> + Send + 'static,
    {
        let connection = self.connection.clone();
        let account = self.account.clone();
        tokio::task::spawn_blocking(move || job(&mut connection.lock(), &account))
            .await
            .map_err(failed)?
            .map_err(failed)
    }
}

#[async_trait]
impl Backend for Sqlite {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        let raw = self
            .run(|db, account| {
                db.query_row("SELECT data FROM soblend_creds WHERE account = ?1", [account], |row| {
                    row.get::<_, String>(0)
                })
                .optional()
            })
            .await?;
        raw.map(|json| serde_json::from_str(&json).map_err(Error::from)).transpose()
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        let json = serde_json::to_string(creds)?;
        self.run(move |db, account| {
            db.execute(
                "INSERT INTO soblend_creds (account, data) VALUES (?1, ?2)
                    ON CONFLICT (account) DO UPDATE SET data = excluded.data",
                params![account, json],
            )
            .map(drop)
        })
        .await
    }

    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        let key = key.to_owned();
        self.run(move |db, account| {
            db.prepare_cached("SELECT value FROM soblend_keys WHERE account = ?1 AND kind = ?2 AND key = ?3")?
                .query_row(params![account, kind.code(), key], |row| row.get(0))
                .optional()
        })
        .await
    }

    async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
        self.run(move |db, account| {
            db.prepare_cached("SELECT key FROM soblend_keys WHERE account = ?1 AND kind = ?2")?
                .query_map(params![account, kind.code()], |row| row.get(0))?
                .collect()
        })
        .await
    }

    async fn apply(&self, writes: &[KeyWrite]) -> Result<()> {
        let writes = writes.to_vec();
        self.run(move |db, account| {
            let tx = db.transaction()?;
            {
                let mut upsert = tx.prepare_cached(UPSERT)?;
                let mut delete =
                    tx.prepare_cached("DELETE FROM soblend_keys WHERE account = ?1 AND kind = ?2 AND key = ?3")?;
                for write in &writes {
                    match write {
                        KeyWrite::Set(kind, key, value) => upsert.execute(params![account, kind.code(), key, value])?,
                        KeyWrite::Remove(kind, key) => delete.execute(params![account, kind.code(), key])?,
                        KeyWrite::Clear => {
                            tx.execute("DELETE FROM soblend_keys WHERE account = ?1", [account])?;
                            tx.execute("DELETE FROM soblend_creds WHERE account = ?1", [account])?
                        }
                    };
                }
            }
            tx.commit()
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{batched, Store};

    #[tokio::test]
    async fn sqlite_round_trip() {
        let store = SqliteStore::in_memory().unwrap();
        batched::tests::exercise(&store).await;
    }

    #[tokio::test]
    async fn sqlite_reopen() {
        let dir = std::env::temp_dir().join(format!("soblendwa-sqlite-{}", crate::crypto::random_u32()));
        let path = dir.join("auth.db");
        {
            let bot = SqliteStore::open_account(&path, "bot").await.unwrap();
            let other = SqliteStore::open_account(&path, "other").await.unwrap();
            bot.save_creds(&Creds::generate()).await.unwrap();
            bot.set(KeyKind::Session, "5215500000000.0", vec![7; 64]).await.unwrap();
            other.set(KeyKind::Session, "5215500000000.0", vec![9]).await.unwrap();
            bot.flush().await.unwrap();
            other.flush().await.unwrap();
        }
        let bot = SqliteStore::open_account(&path, "bot").await.unwrap();
        assert!(bot.load_creds().await.unwrap().is_some());
        assert_eq!(bot.get(KeyKind::Session, "5215500000000.0").await.unwrap(), Some(vec![7; 64]));
        let other = SqliteStore::open_account(&path, "other").await.unwrap();
        assert!(other.load_creds().await.unwrap().is_none());
        assert_eq!(other.get(KeyKind::Session, "5215500000000.0").await.unwrap(), Some(vec![9]));
        drop((bot, other));
        let _ = std::fs::remove_dir_all(dir);
    }
}
