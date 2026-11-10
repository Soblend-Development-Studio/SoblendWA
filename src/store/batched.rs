use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};

use super::{Creds, KeyKind, Store};
use crate::error::{Error, Result};

const MAX_BATCH: usize = 4096;
const RETRY_DELAYS: [Duration; 4] =
    [Duration::from_millis(50), Duration::from_millis(250), Duration::from_secs(1), Duration::from_secs(5)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyWrite {
    Set(KeyKind, String, Vec<u8>),
    Remove(KeyKind, String),
    Clear,
}

#[async_trait]
pub trait Backend: Send + Sync + 'static {
    async fn load_creds(&self) -> Result<Option<Creds>>;

    async fn save_creds(&self, creds: &Creds) -> Result<()>;

    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>>;

    async fn apply(&self, writes: &[KeyWrite]) -> Result<()>;

    async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
        Err(Error::Store(format!("this backend cannot list its {} keys", kind.as_str())))
    }
}

enum Command {
    Write(KeyWrite),
    Flush(oneshot::Sender<Result<()>>),
}

pub(crate) fn coalesce(writes: Vec<KeyWrite>) -> Vec<KeyWrite> {
    let mut cleared = false;
    let mut latest: HashMap<(KeyKind, String), KeyWrite> = HashMap::with_capacity(writes.len());
    for write in writes {
        match write {
            KeyWrite::Clear => {
                cleared = true;
                latest.clear();
            }
            KeyWrite::Set(kind, ref key, _) | KeyWrite::Remove(kind, ref key) => {
                latest.insert((kind, key.clone()), write);
            }
        }
    }
    let mut out = Vec::with_capacity(latest.len() + usize::from(cleared));
    if cleared {
        out.push(KeyWrite::Clear);
    }
    out.extend(latest.into_values());
    out
}

pub struct BatchedStore<B: Backend> {
    backend: Arc<B>,
    cache: DashMap<(KeyKind, String), Option<Vec<u8>>>,
    epoch: AtomicU64,
    queue: mpsc::UnboundedSender<Command>,
}

impl<B: Backend> BatchedStore<B> {
    pub fn new(backend: B) -> Self {
        let backend = Arc::new(backend);
        let (queue, commands) = mpsc::unbounded_channel();
        tokio::spawn(write_loop(backend.clone(), commands));
        BatchedStore { backend, cache: DashMap::new(), epoch: AtomicU64::new(0), queue }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    fn enqueue(&self, write: KeyWrite) -> Result<()> {
        self.queue.send(Command::Write(write)).map_err(|_| Error::Store("store writer has stopped".into()))
    }
}

async fn write_loop<B: Backend>(backend: Arc<B>, mut commands: mpsc::UnboundedReceiver<Command>) {
    let mut writes = Vec::new();
    let mut waiters = Vec::new();
    while let Some(first) = commands.recv().await {
        let mut next = Some(first);
        while let Some(command) = next.take() {
            match command {
                Command::Write(write) => writes.push(write),
                Command::Flush(waiter) => waiters.push(waiter),
            }
            if writes.len() < MAX_BATCH {
                next = commands.try_recv().ok();
            }
        }
        let batch = coalesce(std::mem::take(&mut writes));
        let mut outcome = Ok(());
        if !batch.is_empty() {
            outcome = backend.apply(&batch).await;
            for delay in RETRY_DELAYS {
                let Err(err) = &outcome else { break };
                tracing::warn!(%err, writes = batch.len(), "store write failed, retrying");
                tokio::time::sleep(delay).await;
                outcome = backend.apply(&batch).await;
            }
            if let Err(err) = &outcome {
                tracing::error!(%err, writes = batch.len(), "store write dropped after retries");
            }
        }
        for waiter in waiters.drain(..) {
            let reply = match &outcome {
                Ok(()) => Ok(()),
                Err(err) => Err(Error::Store(err.to_string())),
            };
            let _ = waiter.send(reply);
        }
    }
}

#[async_trait]
impl<B: Backend> Store for BatchedStore<B> {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        self.backend.load_creds().await
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        self.backend.save_creds(creds).await
    }

    async fn get(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        let slot = (kind, key.to_owned());
        if let Some(hit) = self.cache.get(&slot) {
            return Ok(hit.clone());
        }
        let epoch = self.epoch.load(Ordering::Acquire);
        let value = self.backend.fetch(kind, key).await?;
        if self.epoch.load(Ordering::Acquire) == epoch {
            return Ok(self.cache.entry(slot).or_insert(value).clone());
        }
        Ok(self.cache.get(&slot).and_then(|v| v.clone()))
    }

    async fn set(&self, kind: KeyKind, key: &str, value: Vec<u8>) -> Result<()> {
        self.cache.insert((kind, key.to_owned()), Some(value.clone()));
        self.enqueue(KeyWrite::Set(kind, key.to_owned(), value))
    }

    async fn remove(&self, kind: KeyKind, key: &str) -> Result<()> {
        self.cache.insert((kind, key.to_owned()), None);
        self.enqueue(KeyWrite::Remove(kind, key.to_owned()))
    }

    async fn clear(&self) -> Result<()> {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        self.cache.clear();
        self.enqueue(KeyWrite::Clear)?;
        self.flush().await
    }

    async fn flush(&self) -> Result<()> {
        let (reply, done) = oneshot::channel();
        self.queue.send(Command::Flush(reply)).map_err(|_| Error::Store("store writer has stopped".into()))?;
        done.await.map_err(|_| Error::Store("store writer has stopped".into()))?
    }

    async fn keys(&self, kind: KeyKind) -> Result<Vec<String>> {
        self.flush().await?;
        self.backend.list(kind).await
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use parking_lot::Mutex;

    use super::*;

    #[derive(Default)]
    pub(crate) struct Recorder {
        pub creds: Mutex<Option<Creds>>,
        pub rows: Mutex<HashMap<(KeyKind, String), Vec<u8>>>,
        pub batches: Mutex<Vec<usize>>,
        pub fetches: AtomicU64,
    }

    #[async_trait]
    impl Backend for Recorder {
        async fn load_creds(&self) -> Result<Option<Creds>> {
            Ok(self.creds.lock().clone())
        }

        async fn save_creds(&self, creds: &Creds) -> Result<()> {
            *self.creds.lock() = Some(creds.clone());
            Ok(())
        }

        async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            Ok(self.rows.lock().get(&(kind, key.to_owned())).cloned())
        }

        async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
            Ok(self.rows.lock().keys().filter(|(k, _)| *k == kind).map(|(_, key)| key.clone()).collect())
        }

        async fn apply(&self, writes: &[KeyWrite]) -> Result<()> {
            let mut rows = self.rows.lock();
            for write in writes {
                match write {
                    KeyWrite::Set(kind, key, value) => drop(rows.insert((*kind, key.clone()), value.clone())),
                    KeyWrite::Remove(kind, key) => drop(rows.remove(&(*kind, key.clone()))),
                    KeyWrite::Clear => {
                        rows.clear();
                        *self.creds.lock() = None;
                    }
                }
            }
            self.batches.lock().push(writes.len());
            Ok(())
        }
    }

    #[cfg(any(feature = "mongo", feature = "postgres", feature = "redis"))]
    pub(crate) async fn against_a_server<S, F, Fut>(open: F)
    where
        S: Store,
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = S>,
    {
        let tag = crate::crypto::random_u32();
        let (bot, other) = (format!("soblendwa_test_{tag}"), format!("soblendwa_test_{tag}_otro"));
        let store = open(bot.clone()).await;
        exercise(&store).await;

        let neighbour = open(other.clone()).await;
        store.set(KeyKind::Session, "5215500000000.0", vec![7; 64]).await.unwrap();
        neighbour.set(KeyKind::Session, "5215500000000.0", vec![9]).await.unwrap();
        for i in 0..2_000u32 {
            store.set(KeyKind::PreKey, &i.to_string(), i.to_be_bytes().repeat(8)).await.unwrap();
        }
        store.set(KeyKind::SenderKey, "grande", vec![3; 2 * 1024 * 1024]).await.unwrap();
        store.save_creds(&Creds::generate()).await.unwrap();
        store.flush().await.unwrap();
        neighbour.flush().await.unwrap();

        let reopened = open(bot).await;
        assert!(reopened.load_creds().await.unwrap().is_some());
        assert_eq!(reopened.get(KeyKind::Session, "5215500000000.0").await.unwrap(), Some(vec![7; 64]));
        assert_eq!(reopened.keys(KeyKind::PreKey).await.unwrap().len(), 2_000);
        assert_eq!(reopened.get(KeyKind::PreKey, "1999").await.unwrap(), Some(1999u32.to_be_bytes().repeat(8)));
        assert_eq!(reopened.get(KeyKind::SenderKey, "grande").await.unwrap().map(|v| v.len()), Some(2 * 1024 * 1024));
        let isolated = open(other).await;
        assert!(isolated.load_creds().await.unwrap().is_none());
        assert_eq!(isolated.get(KeyKind::Session, "5215500000000.0").await.unwrap(), Some(vec![9]));

        for store in [&reopened, &isolated] {
            store.clear().await.unwrap();
            store.flush().await.unwrap();
        }
        assert!(open(format!("soblendwa_test_{tag}")).await.keys(KeyKind::PreKey).await.unwrap().is_empty());
    }

    pub(crate) async fn exercise(store: &dyn Store) {
        assert!(store.get(KeyKind::Session, "a").await.unwrap().is_none());
        store.set(KeyKind::Session, "a", vec![1]).await.unwrap();
        store.set(KeyKind::Session, "a", vec![2]).await.unwrap();
        store.set(KeyKind::PreKey, "a", vec![3]).await.unwrap();
        store.set(KeyKind::SenderKey, "gone", vec![4]).await.unwrap();
        store.remove(KeyKind::SenderKey, "gone").await.unwrap();
        store.flush().await.unwrap();
        assert_eq!(store.keys(KeyKind::Session).await.unwrap(), vec!["a".to_owned()]);
        assert!(store.keys(KeyKind::SenderKey).await.unwrap().is_empty());
        assert_eq!(store.get(KeyKind::Session, "a").await.unwrap(), Some(vec![2]));
        assert_eq!(store.get(KeyKind::PreKey, "a").await.unwrap(), Some(vec![3]));
        assert_eq!(store.get(KeyKind::SenderKey, "gone").await.unwrap(), None);
        let creds = Creds::generate();
        store.save_creds(&creds).await.unwrap();
        let loaded = store.load_creds().await.unwrap().unwrap();
        assert_eq!(loaded.registration_id, creds.registration_id);
        store.clear().await.unwrap();
        assert!(store.get(KeyKind::Session, "a").await.unwrap().is_none());
        assert!(store.load_creds().await.unwrap().is_none());
    }

    #[test]
    fn last_write_wins() {
        let writes = vec![
            KeyWrite::Set(KeyKind::Session, "x".into(), vec![1]),
            KeyWrite::Set(KeyKind::Session, "y".into(), vec![1]),
            KeyWrite::Clear,
            KeyWrite::Set(KeyKind::Session, "x".into(), vec![2]),
            KeyWrite::Set(KeyKind::Session, "x".into(), vec![3]),
            KeyWrite::Remove(KeyKind::PreKey, "z".into()),
        ];
        let out = coalesce(writes);
        assert_eq!(out[0], KeyWrite::Clear);
        assert_eq!(out.len(), 3);
        assert!(out.contains(&KeyWrite::Set(KeyKind::Session, "x".into(), vec![3])));
        assert!(!out.iter().any(|w| matches!(w, KeyWrite::Set(_, k, _) if k == "y")));
    }

    #[tokio::test]
    async fn cache_and_batch() {
        let store = BatchedStore::new(Recorder::default());
        exercise(&store).await;
        for i in 0..1000 {
            store.set(KeyKind::Session, "hot", vec![(i % 251) as u8]).await.unwrap();
        }
        store.flush().await.unwrap();
        let rows = store.backend().rows.lock().clone();
        assert_eq!(rows.get(&(KeyKind::Session, "hot".to_owned())), Some(&vec![(999 % 251) as u8]));
        let fetches = store.backend().fetches.load(Ordering::Relaxed);
        for _ in 0..100 {
            store.get(KeyKind::Session, "hot").await.unwrap();
        }
        assert_eq!(store.backend().fetches.load(Ordering::Relaxed), fetches);
        let total: usize = store.backend().batches.lock().iter().sum();
        assert!(total < 1000, "1000 writes to one key should collapse, wrote {total}");
    }
}
