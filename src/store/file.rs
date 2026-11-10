use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::Notify;

use super::memory::KeyTable;
use super::{Creds, KeyKind, Store};
use crate::error::{Error, Result};

const SNAPSHOT_MAGIC: &[u8; 4] = b"SWA1";
const LOG_MAGIC: &[u8; 4] = b"SWL1";
const SNAPSHOT_FILE: &str = "keys.bin";
const LOG_FILE: &str = "keys.log";
const CREDS_FILE: &str = "creds.json";
const FLUSH_DELAY: Duration = Duration::from_millis(50);
const COMPACT_AFTER: u64 = 8 * 1024 * 1024;
const OP_SET: u8 = 1;
const OP_REMOVE: u8 = 2;

#[derive(Default)]
struct State {
    table: KeyTable,
    dirty: HashSet<(KeyKind, String)>,
}

#[derive(Default)]
struct Journal {
    log_len: u64,
    snapshot_len: u64,
}

pub struct FileStore {
    dir: PathBuf,
    state: Mutex<State>,
    creds: Mutex<Option<Creds>>,
    wake: Arc<Notify>,
    journal: tokio::sync::Mutex<Journal>,
}

impl FileStore {
    pub async fn open(dir: impl AsRef<Path>) -> Result<Arc<Self>> {
        let dir = dir.as_ref().to_path_buf();
        tokio::fs::create_dir_all(&dir).await?;

        let mut table = match tokio::fs::read(dir.join(SNAPSHOT_FILE)).await {
            Ok(bytes) => decode_snapshot(&bytes)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => KeyTable::default(),
            Err(err) => return Err(err.into()),
        };
        let replayed = match tokio::fs::read(dir.join(LOG_FILE)).await {
            Ok(bytes) => replay_log(&bytes, &mut table),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
            Err(err) => return Err(err.into()),
        };
        let creds = match tokio::fs::read(dir.join(CREDS_FILE)).await {
            Ok(bytes) => Some(serde_json::from_slice(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err.into()),
        };

        let wake = Arc::new(Notify::new());
        let store = Arc::new(FileStore {
            dir,
            state: Mutex::new(State { table, dirty: HashSet::new() }),
            creds: Mutex::new(creds),
            wake: wake.clone(),
            journal: tokio::sync::Mutex::new(Journal::default()),
        });
        let mut journal = store.journal.lock().await;
        if replayed > 0 {
            store.compact(&mut journal).await?;
        } else {
            journal.snapshot_len = tokio::fs::metadata(store.dir.join(SNAPSHOT_FILE)).await.map_or(0, |m| m.len());
        }
        drop(journal);
        tokio::spawn(flusher(Arc::downgrade(&store), wake));
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    fn pending_records(&self) -> Option<Vec<u8>> {
        let mut state = self.state.lock();
        if state.dirty.is_empty() {
            return None;
        }
        let dirty = std::mem::take(&mut state.dirty);
        let mut out = Vec::new();
        for (kind, key) in dirty {
            match state.table.get(&kind).and_then(|t| t.get(&key)) {
                Some(value) => encode_record(&mut out, OP_SET, kind, &key, Some(value)),
                None => encode_record(&mut out, OP_REMOVE, kind, &key, None),
            }
        }
        Some(out)
    }

    async fn persist(&self) -> Result<()> {
        let mut journal = self.journal.lock().await;
        let Some(records) = self.pending_records() else {
            return Ok(());
        };
        let path = self.dir.join(LOG_FILE);
        let written = records.len() as u64;
        tokio::task::spawn_blocking(move || append_log(&path, &records))
            .await
            .map_err(|err| Error::Store(err.to_string()))??;
        journal.log_len += written;
        if journal.log_len > COMPACT_AFTER.max(journal.snapshot_len * 2) {
            self.compact(&mut journal).await?;
        }
        Ok(())
    }

    async fn compact(&self, journal: &mut Journal) -> Result<()> {
        let snapshot = encode_snapshot(&self.state.lock().table);
        let snapshot_len = snapshot.len() as u64;
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || {
            write_atomic(&dir.join(SNAPSHOT_FILE), &snapshot)?;
            write_atomic(&dir.join(LOG_FILE), LOG_MAGIC)
        })
        .await
        .map_err(|err| Error::Store(err.to_string()))??;
        journal.log_len = 0;
        journal.snapshot_len = snapshot_len;
        Ok(())
    }

    fn mark(&self, kind: KeyKind, key: &str) {
        self.state.lock().dirty.insert((kind, key.to_owned()));
        self.wake.notify_one();
    }
}

impl Drop for FileStore {
    fn drop(&mut self) {
        if let Some(records) = self.pending_records() {
            if let Err(err) = append_log(&self.dir.join(LOG_FILE), &records) {
                tracing::error!(%err, "failed to persist keys while dropping the file store");
            }
        }
        self.wake.notify_one();
    }
}

async fn flusher(store: Weak<FileStore>, wake: Arc<Notify>) {
    loop {
        wake.notified().await;
        tokio::time::sleep(FLUSH_DELAY).await;
        let Some(store) = store.upgrade() else { break };
        if let Err(err) = store.persist().await {
            tracing::error!(%err, "failed to persist keys");
        }
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn append_log(path: &Path, records: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    if file.metadata()?.len() == 0 {
        file.write_all(LOG_MAGIC)?;
    }
    file.write_all(records)?;
    Ok(())
}

fn encode_record(out: &mut Vec<u8>, op: u8, kind: KeyKind, key: &str, value: Option<&Vec<u8>>) {
    out.push(op);
    out.push(kind.code());
    out.extend_from_slice(&(key.len() as u16).to_le_bytes());
    out.extend_from_slice(key.as_bytes());
    if let Some(value) = value {
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
    }
}

fn encode_snapshot(keys: &KeyTable) -> Vec<u8> {
    let size: usize = keys.values().flat_map(|t| t.iter()).map(|(k, v)| 7 + k.len() + v.len()).sum();
    let mut out = Vec::with_capacity(SNAPSHOT_MAGIC.len() + size);
    out.extend_from_slice(SNAPSHOT_MAGIC);
    for (kind, table) in keys {
        for (key, value) in table {
            out.push(kind.code());
            out.extend_from_slice(&(key.len() as u16).to_le_bytes());
            out.extend_from_slice(key.as_bytes());
            out.extend_from_slice(&(value.len() as u32).to_le_bytes());
            out.extend_from_slice(value);
        }
    }
    out
}

fn corrupt() -> Error {
    Error::Store("keys file is corrupt".into())
}

struct Cursor<'a> {
    rest: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.rest.len() < len {
            return Err(corrupt());
        }
        let (head, tail) = self.rest.split_at(len);
        self.rest = tail;
        Ok(head)
    }

    fn u16(&mut self) -> Result<usize> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("two bytes")) as usize)
    }

    fn u32(&mut self) -> Result<usize> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("four bytes")) as usize)
    }

    fn kind(&mut self) -> Result<KeyKind> {
        KeyKind::from_code(self.take(1)?[0]).ok_or_else(corrupt)
    }

    fn key(&mut self) -> Result<String> {
        let len = self.u16()?;
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| corrupt())
    }

    fn value(&mut self) -> Result<Vec<u8>> {
        let len = self.u32()?;
        Ok(self.take(len)?.to_vec())
    }
}

fn decode_snapshot(data: &[u8]) -> Result<KeyTable> {
    let mut cursor = Cursor { rest: data.strip_prefix(SNAPSHOT_MAGIC.as_slice()).ok_or_else(corrupt)? };
    let mut table = KeyTable::default();
    while !cursor.rest.is_empty() {
        let kind = cursor.kind()?;
        let key = cursor.key()?;
        let value = cursor.value()?;
        table.entry(kind).or_default().insert(key, value);
    }
    Ok(table)
}

fn replay_log(data: &[u8], table: &mut KeyTable) -> usize {
    let Some(body) = data.strip_prefix(LOG_MAGIC.as_slice()) else { return 0 };
    let mut cursor = Cursor { rest: body };
    let mut applied = 0;
    while !cursor.rest.is_empty() {
        let record = (|| -> Result<()> {
            let op = cursor.take(1)?[0];
            let kind = cursor.kind()?;
            let key = cursor.key()?;
            match op {
                OP_SET => {
                    let value = cursor.value()?;
                    table.entry(kind).or_default().insert(key, value);
                }
                OP_REMOVE => {
                    if let Some(entries) = table.get_mut(&kind) {
                        entries.remove(&key);
                    }
                }
                _ => return Err(corrupt()),
            }
            Ok(())
        })();
        if record.is_err() {
            tracing::warn!("ignoring a torn record at the end of the key log");
            break;
        }
        applied += 1;
    }
    applied
}

#[async_trait]
impl Store for FileStore {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        Ok(self.creds.lock().clone())
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        let json = serde_json::to_vec_pretty(creds)?;
        *self.creds.lock() = Some(creds.clone());
        let path = self.dir.join(CREDS_FILE);
        tokio::task::spawn_blocking(move || write_atomic(&path, &json))
            .await
            .map_err(|err| Error::Store(err.to_string()))?
    }

    async fn get(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().table.get(&kind).and_then(|table| table.get(key)).cloned())
    }

    async fn set(&self, kind: KeyKind, key: &str, value: Vec<u8>) -> Result<()> {
        self.state.lock().table.entry(kind).or_default().insert(key.to_owned(), value);
        self.mark(kind, key);
        Ok(())
    }

    async fn remove(&self, kind: KeyKind, key: &str) -> Result<()> {
        let removed = self.state.lock().table.get_mut(&kind).and_then(|table| table.remove(key)).is_some();
        if removed {
            self.mark(kind, key);
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut journal = self.journal.lock().await;
        {
            let mut state = self.state.lock();
            state.table.clear();
            state.dirty.clear();
        }
        *self.creds.lock() = None;
        for name in [SNAPSHOT_FILE, LOG_FILE, CREDS_FILE] {
            match tokio::fs::remove_file(self.dir.join(name)).await {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
                _ => {}
            }
        }
        *journal = Journal::default();
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        self.persist().await
    }

    async fn keys(&self, kind: KeyKind) -> Result<Vec<String>> {
        Ok(self.state.lock().table.get(&kind).map(|table| table.keys().cloned().collect()).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("soblendwa-test-{}", crate::crypto::random_u32()))
    }

    #[tokio::test]
    async fn survives_reopen() {
        let dir = temp_dir();
        {
            let store = FileStore::open(&dir).await.unwrap();
            store.set(KeyKind::Session, "123.0", vec![1, 2, 3]).await.unwrap();
            store.set(KeyKind::PreKey, "7", vec![9; 32]).await.unwrap();
            store.set(KeyKind::PreKey, "8", vec![8; 32]).await.unwrap();
            store.remove(KeyKind::PreKey, "8").await.unwrap();
            store.save_creds(&Creds::generate()).await.unwrap();
            store.flush().await.unwrap();
            store.set(KeyKind::Session, "123.0", vec![4, 5, 6]).await.unwrap();
        }
        let store = FileStore::open(&dir).await.unwrap();
        assert_eq!(store.get(KeyKind::Session, "123.0").await.unwrap(), Some(vec![4, 5, 6]));
        assert_eq!(store.get(KeyKind::PreKey, "7").await.unwrap(), Some(vec![9; 32]));
        assert_eq!(store.get(KeyKind::PreKey, "8").await.unwrap(), None);
        assert!(store.load_creds().await.unwrap().is_some());
        store.clear().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn torn_tail() {
        let mut log = LOG_MAGIC.to_vec();
        encode_record(&mut log, OP_SET, KeyKind::Session, "a.0", Some(&vec![1, 2]));
        encode_record(&mut log, OP_SET, KeyKind::Session, "b.0", Some(&vec![3, 4]));
        log.truncate(log.len() - 1);
        let mut table = KeyTable::default();
        assert_eq!(replay_log(&log, &mut table), 1);
        assert_eq!(table[&KeyKind::Session]["a.0"], vec![1, 2]);
        assert!(!table[&KeyKind::Session].contains_key("b.0"));
    }
}
