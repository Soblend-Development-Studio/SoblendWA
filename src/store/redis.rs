use ::redis::aio::ConnectionManager;
use async_trait::async_trait;

use super::batched::{Backend, BatchedStore, KeyWrite};
use super::{Creds, KeyKind};
use crate::error::{Error, Result};

fn failed(err: impl std::fmt::Display) -> Error {
    Error::Store(format!("redis: {err}"))
}

pub struct Redis {
    connection: ConnectionManager,
    prefix: String,
}

pub type RedisStore = BatchedStore<Redis>;

impl BatchedStore<Redis> {
    pub async fn connect(url: &str) -> Result<Self> {
        Self::connect_account(url, "default").await
    }

    pub async fn connect_account(url: &str, account: &str) -> Result<Self> {
        let client = ::redis::Client::open(url).map_err(failed)?;
        let connection = ConnectionManager::new(client).await.map_err(failed)?;
        Ok(BatchedStore::new(Redis { connection, prefix: format!("soblendwa:{account}") }))
    }
}

impl Redis {
    fn creds_key(&self) -> String {
        format!("{}:creds", self.prefix)
    }

    fn table(&self, kind: KeyKind) -> String {
        format!("{}:{}", self.prefix, kind.as_str())
    }
}

#[async_trait]
impl Backend for Redis {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        let raw: Option<String> = ::redis::cmd("GET")
            .arg(self.creds_key())
            .query_async(&mut self.connection.clone())
            .await
            .map_err(failed)?;
        raw.map(|json| serde_json::from_str(&json).map_err(Error::from)).transpose()
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        ::redis::cmd("SET")
            .arg(self.creds_key())
            .arg(serde_json::to_string(creds)?)
            .query_async::<()>(&mut self.connection.clone())
            .await
            .map_err(failed)
    }

    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        ::redis::cmd("HGET")
            .arg(self.table(kind))
            .arg(key)
            .query_async(&mut self.connection.clone())
            .await
            .map_err(failed)
    }

    async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
        ::redis::cmd("HKEYS").arg(self.table(kind)).query_async(&mut self.connection.clone()).await.map_err(failed)
    }

    async fn apply(&self, writes: &[KeyWrite]) -> Result<()> {
        let mut pipe = ::redis::pipe();
        pipe.atomic();
        for write in writes {
            match write {
                KeyWrite::Set(kind, key, value) => pipe.cmd("HSET").arg(self.table(*kind)).arg(key).arg(value).ignore(),
                KeyWrite::Remove(kind, key) => pipe.cmd("HDEL").arg(self.table(*kind)).arg(key).ignore(),
                KeyWrite::Clear => {
                    let mut keys: Vec<String> = KeyKind::ALL.into_iter().map(|kind| self.table(kind)).collect();
                    keys.push(self.creds_key());
                    pipe.cmd("DEL").arg(keys).ignore()
                }
            };
        }
        pipe.query_async::<()>(&mut self.connection.clone()).await.map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::batched;

    #[tokio::test]
    #[ignore = "set SOBLEND_REDIS_URL to a Redis you can write to"]
    async fn redis_roundtrip() {
        let url = std::env::var("SOBLEND_REDIS_URL").unwrap();
        batched::tests::against_a_server(|account| {
            let url = url.clone();
            async move { RedisStore::connect_account(&url, &account).await.unwrap() }
        })
        .await;
    }
}
