#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod binary;
pub mod chats;
pub mod crypto;
pub mod error;
pub mod events;
pub mod interactive;
pub mod jid;
pub mod json;
pub mod message;
pub mod proto;
pub mod signal;
pub mod store;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

mod client;
mod noise;
mod serde_b64;

pub use client::{
    fetch_latest_version, guess_mimetype, About, BotInfo, BusinessProfile, ChatModification, ChatPresence, Client,
    ClientBuilder, Config, DisappearingMode, Downloadable, EventStream, GroupInfo, GroupParticipant, JoinRequest,
    LinkPreview, LinkPreviewConfig, LinkPreviewFetcher, LinkThumbnail, LinkedGroup, MediaKind, MediaOptions,
    NewsletterInfo, NewsletterMessage, OnWhatsApp, ParticipantAction, ParticipantResult, PendingSend, Platform,
    PreviewChoice, PrivacySetting, PrivacyValue, Proxy, ProxyKind, ReachoutTimelock, SendLimit, SendOptions,
    SendTimings, Stats, UploadedMedia, COLLECTIONS, DEFAULT_VERSION,
};
pub use error::{Error, Result};

pub mod business {
    pub use crate::client::{
        BusinessHours, BusinessProfileUpdate, Catalog, CatalogImage, OpeningHours, Order, OrderItem, Product,
        ProductCollection, ProductDraft, ProductImage, Weekday,
    };
}

pub mod imaging {
    pub use crate::client::{image_dimensions, profile_picture, thumbnail};
}
pub use chats::ChatStore;
pub use events::{Event, MessageEvent, MessageInfo};
pub use jid::Jid;
pub use prost;
pub use proto::Message;
pub use store::{Backend, BatchedStore, FileStore, MemoryStore, Store};

pub const NAME: &str = "SoblendWA";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = "Soblend Development Studio";

pub fn credits() -> String {
    format!("{NAME} v{VERSION} · crafted by {AUTHOR}")
}
