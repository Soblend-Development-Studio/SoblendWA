use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

use super::batched::{Backend, BatchedStore, KeyWrite};
use super::{Creds, KeyKind};
use crate::error::{Error, Result};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS soblend_creds (
        account TEXT PRIMARY KEY,
        data TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS soblend_keys (
        account TEXT NOT NULL,
        kind SMALLINT NOT NULL,
        key TEXT NOT NULL,
        value BYTEA NOT NULL,
        PRIMARY KEY (account, kind, key)
    );
";

fn failed(err: impl std::fmt::Display) -> Error {
    Error::Store(format!("postgres: {err}"))
}

pub struct Postgres {
    client: Mutex<Client>,
    account: String,
}

pub type PostgresStore = BatchedStore<Postgres>;

impl BatchedStore<Postgres> {
    pub async fn connect(url: &str) -> Result<Self> {
        Self::connect_account(url, "default").await
    }

    pub async fn connect_account(url: &str, account: &str) -> Result<Self> {
        let (client, connection) = tokio_postgres::connect(url, NoTls).await.map_err(failed)?;
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                tracing::error!(%err, "postgres connection closed");
            }
        });
        Self::from_client(client, account).await
    }

    pub async fn from_client(client: Client, account: &str) -> Result<Self> {
        client.batch_execute(SCHEMA).await.map_err(failed)?;
        Ok(BatchedStore::new(Postgres { client: Mutex::new(client), account: account.to_owned() }))
    }
}

#[async_trait]
impl Backend for Postgres {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        let client = self.client.lock().await;
        let row = client
            .query_opt("SELECT data FROM soblend_creds WHERE account = $1", &[&self.account])
            .await
            .map_err(failed)?;
        row.map(|row| serde_json::from_str(row.get::<_, &str>(0)).map_err(Error::from)).transpose()
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        let json = serde_json::to_string(creds)?;
        let client = self.client.lock().await;
        client
            .execute(
                "INSERT INTO soblend_creds (account, data) VALUES ($1, $2)
                    ON CONFLICT (account) DO UPDATE SET data = EXCLUDED.data",
                &[&self.account, &json],
            )
            .await
            .map(drop)
            .map_err(failed)
    }

    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        let client = self.client.lock().await;
        let row = client
            .query_opt(
                "SELECT value FROM soblend_keys WHERE account = $1 AND kind = $2 AND key = $3",
                &[&self.account, &i16::from(kind.code()), &key],
            )
            .await
            .map_err(failed)?;
        Ok(row.map(|row| row.get(0)))
    }

    async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
        let client = self.client.lock().await;
        let rows = client
            .query(
                "SELECT key FROM soblend_keys WHERE account = $1 AND kind = $2",
                &[&self.account, &i16::from(kind.code())],
            )
            .await
            .map_err(failed)?;
        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    async fn apply(&self, writes: &[KeyWrite]) -> Result<()> {
        let mut set_kinds = Vec::new();
        let mut set_keys = Vec::new();
        let mut set_values = Vec::new();
        let mut removed_kinds = Vec::new();
        let mut removed_keys = Vec::new();
        let mut cleared = false;
        for write in writes {
            match write {
                KeyWrite::Set(kind, key, value) => {
                    set_kinds.push(i16::from(kind.code()));
                    set_keys.push(key.as_str());
                    set_values.push(value.as_slice());
                }
                KeyWrite::Remove(kind, key) => {
                    removed_kinds.push(i16::from(kind.code()));
                    removed_keys.push(key.as_str());
                }
                KeyWrite::Clear => cleared = true,
            }
        }
        let mut client = self.client.lock().await;
        let tx = client.transaction().await.map_err(failed)?;
        if cleared {
            tx.execute("DELETE FROM soblend_keys WHERE account = $1", &[&self.account]).await.map_err(failed)?;
            tx.execute("DELETE FROM soblend_creds WHERE account = $1", &[&self.account]).await.map_err(failed)?;
        }
        if !removed_keys.is_empty() {
            tx.execute(
                "DELETE FROM soblend_keys WHERE account = $1
                    AND (kind, key) IN (SELECT * FROM UNNEST($2::smallint[], $3::text[]))",
                &[&self.account, &removed_kinds, &removed_keys],
            )
            .await
            .map_err(failed)?;
        }
        if !set_keys.is_empty() {
            tx.execute(
                "INSERT INTO soblend_keys (account, kind, key, value)
                    SELECT $1, * FROM UNNEST($2::smallint[], $3::text[], $4::bytea[])
                    ON CONFLICT (account, kind, key) DO UPDATE SET value = EXCLUDED.value",
                &[&self.account, &set_kinds, &set_keys, &set_values],
            )
            .await
            .map_err(failed)?;
        }
        tx.commit().await.map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::batched;

    #[tokio::test]
    #[ignore = "set SOBLEND_POSTGRES_URL to a PostgreSQL you can write to"]
    async fn postgres_roundtrip() {
        let url = std::env::var("SOBLEND_POSTGRES_URL").unwrap();
        batched::tests::against_a_server(|account| {
            let url = url.clone();
            async move { PostgresStore::connect_account(&url, &account).await.unwrap() }
        })
        .await;
    }
}
