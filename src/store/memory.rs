use std::collections::HashMap;

use async_trait::async_trait;
use parking_lot::Mutex;

use super::{Creds, KeyKind, Store};
use crate::error::Result;

pub(crate) type KeyTable = HashMap<KeyKind, HashMap<String, Vec<u8>>>;

#[derive(Default)]
pub struct MemoryStore {
    creds: Mutex<Option<Creds>>,
    keys: Mutex<KeyTable>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Store for MemoryStore {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        Ok(self.creds.lock().clone())
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        *self.creds.lock() = Some(creds.clone());
        Ok(())
    }

    async fn get(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.keys.lock().get(&kind).and_then(|table| table.get(key)).cloned())
    }

    async fn set(&self, kind: KeyKind, key: &str, value: Vec<u8>) -> Result<()> {
        self.keys.lock().entry(kind).or_default().insert(key.to_owned(), value);
        Ok(())
    }

    async fn remove(&self, kind: KeyKind, key: &str) -> Result<()> {
        if let Some(table) = self.keys.lock().get_mut(&kind) {
            table.remove(key);
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.keys.lock().clear();
        *self.creds.lock() = None;
        Ok(())
    }

    async fn keys(&self, kind: KeyKind) -> Result<Vec<String>> {
        Ok(self.keys.lock().get(&kind).map(|table| table.keys().cloned().collect()).unwrap_or_default())
    }
}
