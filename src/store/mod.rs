mod baileys;
mod batched;
mod creds;
mod file;
mod memory;
#[cfg(feature = "mongo")]
pub(crate) mod mongo;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(any(feature = "sqlite", feature = "mongo"))]
mod zapo;

use async_trait::async_trait;

#[cfg(feature = "mongo")]
pub use self::mongo::{Mongo, MongoStore};
#[cfg(feature = "redis")]
pub use self::redis::{Redis, RedisStore};
pub use baileys::{creds_from_baileys, import_baileys, BaileysImport};
pub use batched::{Backend, BatchedStore, KeyWrite};
pub use creds::{Creds, SignedPreKey};
pub use file::FileStore;
pub use memory::MemoryStore;
#[cfg(feature = "postgres")]
pub use postgres::{Postgres, PostgresStore};
#[cfg(feature = "sqlite")]
pub use sqlite::{Sqlite, SqliteStore};
#[cfg(feature = "mongo")]
pub use zapo::{export_zapo_mongo, import_zapo_mongo, ZapoMongo};
#[cfg(feature = "sqlite")]
pub use zapo::{export_zapo_sqlite, import_zapo_sqlite, zapo_sessions};
#[cfg(any(feature = "sqlite", feature = "mongo"))]
pub use zapo::{ZapoExport, ZapoImport};

use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyKind {
    Session,
    PreKey,
    SenderKey,
    SenderKeyMemory,
    LidMapping,
    DeviceList,
    GroupMetadata,
    PollSecret,
    AppStateKey,
    AppStateVersion,
    TcToken,
    MediaCache,
    Outbox,
    RecentSend,
}

impl KeyKind {
    pub const ALL: [KeyKind; 14] = [
        KeyKind::Session,
        KeyKind::PreKey,
        KeyKind::SenderKey,
        KeyKind::SenderKeyMemory,
        KeyKind::LidMapping,
        KeyKind::DeviceList,
        KeyKind::GroupMetadata,
        KeyKind::PollSecret,
        KeyKind::AppStateKey,
        KeyKind::AppStateVersion,
        KeyKind::TcToken,
        KeyKind::MediaCache,
        KeyKind::Outbox,
        KeyKind::RecentSend,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            KeyKind::Session => "session",
            KeyKind::PreKey => "pre-key",
            KeyKind::SenderKey => "sender-key",
            KeyKind::SenderKeyMemory => "sender-key-memory",
            KeyKind::LidMapping => "lid-mapping",
            KeyKind::DeviceList => "device-list",
            KeyKind::GroupMetadata => "group-metadata",
            KeyKind::PollSecret => "poll-secret",
            KeyKind::AppStateKey => "app-state-sync-key",
            KeyKind::AppStateVersion => "app-state-sync-version",
            KeyKind::TcToken => "tctoken",
            KeyKind::MediaCache => "media-cache",
            KeyKind::Outbox => "outbox",
            KeyKind::RecentSend => "recent-send",
        }
    }

    pub(crate) fn code(self) -> u8 {
        match self {
            KeyKind::Session => 1,
            KeyKind::PreKey => 2,
            KeyKind::SenderKey => 3,
            KeyKind::SenderKeyMemory => 4,
            KeyKind::LidMapping => 5,
            KeyKind::DeviceList => 6,
            KeyKind::GroupMetadata => 7,
            KeyKind::PollSecret => 8,
            KeyKind::AppStateKey => 9,
            KeyKind::AppStateVersion => 10,
            KeyKind::TcToken => 11,
            KeyKind::MediaCache => 12,
            KeyKind::Outbox => 13,
            KeyKind::RecentSend => 14,
        }
    }

    pub(crate) fn from_code(code: u8) -> Option<Self> {
        KeyKind::ALL.into_iter().find(|kind| kind.code() == code)
    }
}

#[async_trait]
pub trait Store: Send + Sync + 'static {
    async fn load_creds(&self) -> Result<Option<Creds>>;

    async fn save_creds(&self, creds: &Creds) -> Result<()>;

    async fn get(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>>;

    async fn set(&self, kind: KeyKind, key: &str, value: Vec<u8>) -> Result<()>;

    async fn remove(&self, kind: KeyKind, key: &str) -> Result<()>;

    async fn clear(&self) -> Result<()>;

    async fn flush(&self) -> Result<()> {
        Ok(())
    }

    async fn keys(&self, kind: KeyKind) -> Result<Vec<String>> {
        Err(crate::error::Error::Store(format!("this store cannot list its {} keys", kind.as_str())))
    }
}
