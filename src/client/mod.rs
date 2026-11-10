mod account;
mod appstate;
mod business;
mod community;
mod connection;
mod enrich;
mod groups;
mod guard;
mod handlers;
mod history;
mod link_preview;
mod media;
mod media_cache;
mod newsletter;
mod outbox;
mod pairing;
mod peer;
mod polls;
mod prekeys;
mod presence;
mod profile;
mod proxy;
mod receive;
mod reporting;
mod send;
mod streaming;
mod tctoken;
mod usync;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use dashmap::{DashMap, DashSet};
use parking_lot::{Mutex, RwLock};
use tokio::sync::{mpsc, oneshot, watch};

pub use account::{BotInfo, DisappearingMode, ReachoutTimelock};
pub use appstate::{ChatModification, COLLECTIONS};
pub use business::{
    BusinessHours, BusinessProfileUpdate, Catalog, CatalogImage, OpeningHours, Order, OrderItem, Product,
    ProductCollection, ProductDraft, ProductImage, Weekday,
};
pub use community::LinkedGroup;
pub use enrich::{image_dimensions, profile_picture, thumbnail};
pub use groups::{GroupInfo, GroupParticipant, JoinRequest, ParticipantAction, ParticipantResult};
pub use guard::{SendLimit, SendTimings, Spread, Stats, TimingSummary};
pub use link_preview::{LinkPreview, LinkPreviewConfig, LinkPreviewFetcher, LinkThumbnail, PreviewChoice};
pub use media::{Downloadable, MediaKind, MediaOptions, UploadedMedia};
pub use newsletter::{NewsletterInfo, NewsletterMessage};
pub use outbox::PendingSend;
pub use presence::ChatPresence;
pub use profile::{About, BusinessProfile, PrivacySetting, PrivacyValue};
pub use proxy::{Proxy, ProxyKind};
pub use send::SendOptions;
pub use streaming::guess_mimetype;
pub use usync::OnWhatsApp;

use crate::binary::{self, Node};
use crate::chats::ChatStore;
use crate::error::{Error, Result};
use crate::events::{DisconnectReason, Event, MessageInfo};
use crate::jid::Jid;
use crate::proto;
use crate::signal::SignalRepository;
use crate::store::{Creds, MemoryStore, Store};

pub const DEFAULT_VERSION: [u32; 3] = [2, 3000, 1052682108];
const WS_URL: &str = "wss://web.whatsapp.com/ws/chat";
const SENT_CACHE_LIMIT: usize = 1024;
const MAX_AWAITING_RETRY: usize = 4096;
const RECENT_JOIN_WINDOW: Duration = Duration::from_secs(5 * 60);
const TIMING_WINDOW: usize = 512;

pub(crate) struct Awaiting {
    chat: Jid,
    sender: Jid,
    error: String,
    since: Instant,
}
const SEEN_MESSAGES: usize = 8192;

pub type EventStream = mpsc::UnboundedReceiver<Event>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Chrome,
    Firefox,
    Safari,
    Edge,
    Opera,
    Desktop,
}

impl Platform {
    pub(crate) fn device_props_type(self) -> i32 {
        match self {
            Platform::Chrome => 1,
            Platform::Firefox => 2,
            Platform::Opera => 4,
            Platform::Safari => 5,
            Platform::Edge => 6,
            Platform::Desktop => 7,
        }
    }

    pub(crate) fn companion_id(self) -> u8 {
        match self {
            Platform::Chrome => 1,
            Platform::Edge => 2,
            Platform::Firefox => 3,
            Platform::Opera => 5,
            Platform::Safari => 6,
            Platform::Desktop => 7,
        }
    }

    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Platform::Chrome => "Chrome",
            Platform::Firefox => "Firefox",
            Platform::Safari => "Safari",
            Platform::Edge => "Edge",
            Platform::Opera => "Opera",
            Platform::Desktop => "Desktop",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub device_name: String,
    pub platform: Platform,
    pub version: Option<[u32; 3]>,
    pub push_name: Option<String>,
    pub country_code: String,
    pub online_on_connect: bool,
    pub auto_reconnect: bool,
    pub connect_timeout: Duration,
    pub query_timeout: Duration,
    pub keepalive_interval: Duration,
    pub workers: usize,
    pub ws_url: String,
    pub proxy: Option<Proxy>,
    pub media_cache_ttl: Option<Duration>,
    pub send_limit: Option<SendLimit>,
    pub outbox: bool,
    pub trust_root: Option<[u8; 32]>,
    pub media_endpoint: Option<String>,
    pub link_previews: LinkPreviewConfig,
    pub retry_deadline: Duration,
}

impl Default for Config {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Config {
            device_name: crate::NAME.to_owned(),
            platform: Platform::Chrome,
            version: None,
            push_name: None,
            country_code: "US".to_owned(),
            online_on_connect: true,
            auto_reconnect: true,
            connect_timeout: Duration::from_secs(20),
            query_timeout: Duration::from_secs(60),
            keepalive_interval: Duration::from_secs(15),
            workers: (cores * 2).clamp(4, 64),
            ws_url: WS_URL.to_owned(),
            proxy: None,
            media_cache_ttl: Some(Duration::from_secs(7 * 24 * 60 * 60)),
            send_limit: Some(SendLimit::default()),
            outbox: true,
            trust_root: None,
            media_endpoint: None,
            link_previews: LinkPreviewConfig::default(),
            retry_deadline: Duration::from_secs(10 * 60),
        }
    }
}

pub struct ClientBuilder {
    config: Config,
    store: Option<Arc<dyn Store>>,
    chat_store: Option<Arc<ChatStore>>,
    media_store: Option<Arc<dyn Store>>,
}

impl ClientBuilder {
    pub fn store(mut self, store: Arc<dyn Store>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn chat_store(mut self, chats: Arc<ChatStore>) -> Self {
        self.chat_store = Some(chats);
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn device_name(mut self, name: impl Into<String>) -> Self {
        self.config.device_name = name.into();
        self
    }

    pub fn platform(mut self, platform: Platform) -> Self {
        self.config.platform = platform;
        self
    }

    pub fn version(mut self, version: [u32; 3]) -> Self {
        self.config.version = Some(version);
        self
    }

    pub fn push_name(mut self, name: impl Into<String>) -> Self {
        self.config.push_name = Some(name.into());
        self
    }

    pub fn country_code(mut self, code: impl Into<String>) -> Self {
        self.config.country_code = code.into();
        self
    }

    pub fn online_on_connect(mut self, online: bool) -> Self {
        self.config.online_on_connect = online;
        self
    }

    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.config.auto_reconnect = enabled;
        self
    }

    pub fn query_timeout(mut self, timeout: Duration) -> Self {
        self.config.query_timeout = timeout;
        self
    }

    pub fn media_cache(mut self, ttl: Option<Duration>) -> Self {
        self.config.media_cache_ttl = ttl;
        self
    }

    pub fn ws_url(mut self, url: impl Into<String>) -> Self {
        self.config.ws_url = url.into();
        self
    }

    pub fn media_endpoint(mut self, origin: impl Into<String>) -> Self {
        self.config.media_endpoint = Some(origin.into().trim_end_matches('/').to_owned());
        self
    }

    pub fn link_previews(mut self, config: LinkPreviewConfig) -> Self {
        self.config.link_previews = config;
        self
    }

    pub fn trust_root(mut self, key: [u8; 32]) -> Self {
        self.config.trust_root = Some(key);
        self
    }

    pub fn send_limit(mut self, limit: Option<SendLimit>) -> Self {
        self.config.send_limit = limit;
        self
    }

    pub fn outbox(mut self, enabled: bool) -> Self {
        self.config.outbox = enabled;
        self
    }

    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.config.proxy = Some(proxy);
        self
    }

    pub fn media_store(mut self, store: Arc<dyn Store>) -> Self {
        self.media_store = Some(store);
        self
    }

    pub fn retry_deadline(mut self, deadline: Duration) -> Self {
        self.config.retry_deadline = deadline;
        self
    }

    pub fn workers(mut self, workers: usize) -> Self {
        self.config.workers = workers.max(1);
        self
    }

    pub async fn build(self) -> Result<(Client, EventStream)> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let store = self.store.unwrap_or_else(|| Arc::new(MemoryStore::new()));
        let creds = match store.load_creds().await? {
            Some(creds) => creds,
            None => {
                let creds = Creds::generate();
                store.save_creds(&creds).await?;
                creds
            }
        };

        let (events, stream) = mpsc::unbounded_channel();
        let (stop, _) = watch::channel(false);
        let workers = self.config.workers.max(1);
        let mut lanes = Vec::with_capacity(workers);
        let mut receivers = Vec::with_capacity(workers);
        for _ in 0..workers {
            let (tx, rx) = mpsc::unbounded_channel();
            lanes.push(tx);
            receivers.push(rx);
        }

        let tag_prefix = format!("{}.{}-", crate::crypto::random_u32() % 10_000, crate::crypto::random_u32() % 10_000);
        let mut http = reqwest::Client::builder().user_agent(concat!("SoblendWA/", env!("CARGO_PKG_VERSION")));
        if let Some(proxy) = &self.config.proxy {
            http = http.proxy(proxy.for_http()?);
        }
        let http = http.build()?;
        let preview_http = link_preview::http_client(&self.config)?;

        let inner = Arc::new(Inner {
            signal: RwLock::new(Arc::new(SignalRepository::new(store.clone(), &creds))),
            creds: RwLock::new(creds),
            config: self.config,
            media_store: self.media_store.unwrap_or_else(|| store.clone()),
            store,
            chat_store: self.chat_store,
            events,
            link: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
            tag_prefix,
            generation: AtomicU64::new(0),
            connected: AtomicBool::new(false),
            active: AtomicBool::new(false),
            running: AtomicBool::new(false),
            stop,
            lanes,
            devices: DashMap::new(),
            refreshing: DashSet::new(),
            groups: DashMap::new(),
            sender_memory: DashMap::new(),
            sent: Mutex::new(SentCache::default()),
            retries: Mutex::new(HashMap::new()),
            media_conn: tokio::sync::Mutex::new(None),
            http,
            preview_http,
            previews: DashMap::new(),
            version: RwLock::new(None),
            pre_key_lock: tokio::sync::Mutex::new(()),
            latency_micros: AtomicU64::new(0),
            app_state_lock: tokio::sync::Mutex::new(()),
            last_messages: DashMap::new(),
            requested_keys: DashSet::new(),
            server_props: tctoken::ServerProps::default(),
            issuing_tokens: DashSet::new(),
            media_retries: DashMap::new(),
            fetch_locks: DashMap::new(),
            awaiting_retry: DashMap::new(),
            lost: DashSet::new(),
            recent_joins: DashMap::new(),
            timing_window: Mutex::new(guard::TimingWindow::default()),
            in_flight: DashSet::new(),
            seen: Mutex::new(SeenMessages::default()),
            counters: guard::Counters::default(),
            limiter: guard::Limiter::default(),
            chat_lanes: DashMap::new(),
            started: Instant::now(),
            upload_times: Mutex::new(HashMap::new()),
        });

        for rx in receivers {
            tokio::spawn(lane_worker(Arc::downgrade(&inner), rx));
        }
        Ok((Client { inner }, stream))
    }
}

async fn lane_worker(inner: Weak<Inner>, mut rx: mpsc::UnboundedReceiver<Node>) {
    while let Some(node) = rx.recv().await {
        let Some(inner) = inner.upgrade() else { break };
        let client = Client { inner };
        client.contained("a message handler", client.handle_message(node)).await;
    }
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Stopped,
    Restart,
    LoggedOut { reason: String, cause: crate::events::LogoutCause, code: Option<u16> },
    Replaced,
    QrTimeout,
    Outdated,
    Blocked { code: u16, reason: String, expires: Option<u64> },
    Lost(String),
}

impl Outcome {
    fn reason(&self) -> DisconnectReason {
        match self {
            Outcome::Blocked { code, .. } => DisconnectReason::AccountBlocked(*code),
            Outcome::Stopped | Outcome::LoggedOut { .. } => DisconnectReason::Manual,
            Outcome::Restart => DisconnectReason::RestartRequired,
            Outcome::Replaced => DisconnectReason::Replaced,
            Outcome::QrTimeout => DisconnectReason::QrTimeout,
            Outcome::Outdated => DisconnectReason::ClientOutdated,
            Outcome::Lost(why) => DisconnectReason::ConnectionLost(why.clone()),
        }
    }
}

pub(crate) struct Link {
    outbound: mpsc::UnboundedSender<Vec<u8>>,
    close: mpsc::UnboundedSender<Outcome>,
}

#[derive(Clone)]
pub(crate) struct SentMessage {
    pub chat: Jid,
    pub message: proto::Message,
    pub sender_address: Option<String>,
    pub sent_at: Instant,
    pub timings: SendTimings,
}

#[derive(Default)]
pub(crate) struct SeenMessages {
    order: VecDeque<String>,
    ids: HashSet<String>,
}

impl SeenMessages {
    pub(crate) fn first_time(&mut self, key: String) -> bool {
        if !self.ids.insert(key.clone()) {
            return false;
        }
        self.order.push_back(key);
        while self.order.len() > SEEN_MESSAGES {
            if let Some(old) = self.order.pop_front() {
                self.ids.remove(&old);
            }
        }
        true
    }
}

#[derive(Default)]
pub(crate) struct SentCache {
    order: VecDeque<String>,
    items: HashMap<String, SentMessage>,
}

impl SentCache {
    fn insert(&mut self, id: String, message: SentMessage) {
        if self.items.insert(id.clone(), message).is_none() {
            self.order.push_back(id);
        }
        while self.order.len() > SENT_CACHE_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.items.remove(&old);
            }
        }
    }

    fn get(&self, id: &str) -> Option<SentMessage> {
        self.items.get(id).cloned()
    }

    pub(crate) fn sent_at(&self, id: &str) -> Option<Instant> {
        self.items.get(id).map(|sent| sent.sent_at)
    }

    pub(crate) fn timings(&self, id: &str) -> Option<SendTimings> {
        self.items.get(id).map(|sent| sent.timings)
    }

    pub(crate) fn note_queued(&mut self, id: &str, queued: Duration) {
        if let Some(sent) = self.items.get_mut(id) {
            sent.timings.queued = queued;
        }
    }
}

pub(crate) struct Inner {
    config: Config,
    store: Arc<dyn Store>,
    media_store: Arc<dyn Store>,
    chat_store: Option<Arc<ChatStore>>,
    creds: RwLock<Creds>,
    signal: RwLock<Arc<SignalRepository>>,
    events: mpsc::UnboundedSender<Event>,
    link: Mutex<Option<Link>>,
    pending: Mutex<HashMap<String, oneshot::Sender<Node>>>,
    counter: AtomicU64,
    tag_prefix: String,
    generation: AtomicU64,
    connected: AtomicBool,
    active: AtomicBool,
    running: AtomicBool,
    stop: watch::Sender<bool>,
    lanes: Vec<mpsc::UnboundedSender<Node>>,
    devices: DashMap<Jid, (i64, Vec<Jid>)>,
    refreshing: DashSet<Jid>,
    groups: DashMap<Jid, GroupInfo>,
    sender_memory: DashMap<Jid, HashSet<String>>,
    sent: Mutex<SentCache>,
    retries: Mutex<HashMap<String, u32>>,
    media_conn: tokio::sync::Mutex<Option<media::MediaConn>>,
    http: reqwest::Client,
    preview_http: reqwest::Client,
    previews: DashMap<String, (Instant, LinkPreview)>,
    version: RwLock<Option<[u32; 3]>>,
    pre_key_lock: tokio::sync::Mutex<()>,
    latency_micros: AtomicU64,
    app_state_lock: tokio::sync::Mutex<()>,
    last_messages: DashMap<Jid, (proto::MessageKey, i64)>,
    requested_keys: DashSet<String>,
    server_props: tctoken::ServerProps,
    issuing_tokens: DashSet<Jid>,
    media_retries: DashMap<String, oneshot::Sender<Node>>,
    fetch_locks: DashMap<Jid, Arc<tokio::sync::Mutex<()>>>,
    awaiting_retry: DashMap<String, Awaiting>,
    lost: DashSet<String>,
    recent_joins: DashMap<Jid, Instant>,
    timing_window: Mutex<guard::TimingWindow>,
    in_flight: DashSet<String>,
    seen: Mutex<SeenMessages>,
    counters: guard::Counters,
    limiter: guard::Limiter,
    chat_lanes: DashMap<Jid, Arc<tokio::sync::Mutex<()>>>,
    started: Instant,
    upload_times: Mutex<HashMap<String, Duration>>,
}

#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<Inner>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder { config: Config::default(), store: None, chat_store: None, media_store: None }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn store(&self) -> &Arc<dyn Store> {
        &self.inner.store
    }

    pub fn chats(&self) -> Option<&Arc<ChatStore>> {
        self.inner.chat_store.as_ref()
    }

    pub fn is_connected(&self) -> bool {
        self.inner.connected.load(Ordering::Acquire)
    }

    pub fn is_logged_in(&self) -> bool {
        self.inner.creds.read().is_logged_in()
    }

    pub fn me(&self) -> Option<Jid> {
        self.inner.creds.read().me.clone()
    }

    pub fn lid(&self) -> Option<Jid> {
        self.inner.creds.read().lid.clone()
    }

    pub fn creds(&self) -> Creds {
        self.inner.creds.read().clone()
    }

    pub async fn connect(&self) -> Result<()> {
        if *self.inner.stop.borrow() {
            let deadline = Instant::now() + Duration::from_secs(10);
            while self.inner.running.load(Ordering::Acquire) && Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        if self.inner.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.inner.stop.send_replace(false);
        let client = self.clone();
        tokio::spawn(async move { client.supervise().await });
        Ok(())
    }

    pub async fn disconnect(&self) {
        self.inner.stop.send_replace(true);
        self.end_link(Outcome::Stopped);
        if let Err(err) = self.inner.store.flush().await {
            tracing::warn!(%err, "failed to flush the store while disconnecting");
        }
        if let Some(chats) = &self.inner.chat_store {
            if let Err(err) = chats.save().await {
                tracing::warn!(%err, "failed to save the chat store while disconnecting");
            }
        }
    }

    pub async fn logout(&self) -> Result<()> {
        if let Some(me) = self.me() {
            let node = Node::new("iq")
                .attr("to", Jid::server())
                .attr("type", "set")
                .attr("xmlns", "md")
                .child_node(Node::new("remove-companion-device").attr("jid", me).attr("reason", "user_initiated"));
            if let Err(err) = self.query(node).await {
                tracing::warn!(%err, "logout request failed, wiping the session anyway");
            }
        }
        self.inner.stop.send_replace(true);
        self.end_link(Outcome::LoggedOut {
            reason: "logged out by the user".into(),
            cause: crate::events::LogoutCause::UserLogout,
            code: None,
        });
        self.reset_session().await
    }

    pub(crate) async fn reset_session(&self) -> Result<()> {
        self.inner.store.clear().await?;
        let creds = Creds::generate();
        self.inner.store.save_creds(&creds).await?;
        *self.inner.signal.write() = Arc::new(SignalRepository::new(self.inner.store.clone(), &creds));
        *self.inner.creds.write() = creds;
        self.inner.devices.clear();
        self.inner.groups.clear();
        self.inner.sender_memory.clear();
        Ok(())
    }

    pub(crate) fn signal(&self) -> Arc<SignalRepository> {
        self.inner.signal.read().clone()
    }

    pub(crate) fn emit(&self, event: Event) {
        let recovered = match &event {
            Event::Message(message) => {
                guard::Counters::bump(&self.inner.counters.received);
                let id = &message.info.id;
                let waiting = self.inner.awaiting_retry.remove(id).map(|(id, _)| id);
                let late = || {
                    let found = self.inner.lost.remove(id);
                    if found.is_some() {
                        guard::Counters::drop_one(&self.inner.counters.lost);
                    }
                    found
                };
                waiting.or_else(late).map(|id| Event::MessageRecovered {
                    id,
                    chat: message.info.chat.clone(),
                    sender: message.info.sender.clone(),
                })
            }
            Event::UndecryptableMessage { .. } => {
                guard::Counters::bump(&self.inner.counters.undecryptable);
                None
            }
            _ => None,
        };
        if let Some(chats) = &self.inner.chat_store {
            chats.ingest(&event);
        }
        let _ = self.inner.events.send(event);
        if let Some(recovered) = recovered {
            guard::Counters::bump(&self.inner.counters.recovered);
            let _ = self.inner.events.send(recovered);
        }
    }

    pub(crate) fn await_retry(&self, info: &MessageInfo, error: &str) {
        if self.inner.awaiting_retry.contains_key(&info.id) {
            return;
        }
        if self.inner.awaiting_retry.len() >= MAX_AWAITING_RETRY {
            tracing::warn!(id = %info.id, "too many messages awaiting a retry, not tracking this one");
            return;
        }
        let awaiting = Awaiting {
            chat: info.chat.clone(),
            sender: info.sender.clone(),
            error: error.to_owned(),
            since: Instant::now(),
        };
        self.inner.awaiting_retry.insert(info.id.clone(), awaiting);
        let client = Arc::downgrade(&self.inner);
        let id = info.id.clone();
        let deadline = self.inner.config.retry_deadline;
        tokio::spawn(async move {
            tokio::time::sleep(deadline).await;
            let Some(inner) = client.upgrade() else { return };
            Client { inner }.give_up_on(&id);
        });
    }

    fn give_up_on(&self, id: &str) {
        let Some((id, awaiting)) = self.inner.awaiting_retry.remove(id) else { return };
        if self.inner.lost.len() >= MAX_AWAITING_RETRY {
            self.inner.lost.clear();
        }
        self.inner.lost.insert(id.clone());
        guard::Counters::bump(&self.inner.counters.lost);
        tracing::warn!(%id, chat = %awaiting.chat, error = %awaiting.error, "gave up waiting for a message to be resent");
        self.emit(Event::MessageLost {
            id,
            chat: awaiting.chat,
            sender: awaiting.sender,
            error: awaiting.error,
            waited: awaiting.since.elapsed(),
        });
    }

    pub(crate) fn announce_join(&self, group: &Jid, author: Option<Jid>, via: crate::events::JoinVia) {
        let now = Instant::now();
        let fresh = match self.inner.recent_joins.get(group) {
            Some(at) => now.duration_since(*at) > RECENT_JOIN_WINDOW,
            None => true,
        };
        if !fresh {
            return;
        }
        if self.inner.recent_joins.len() > 1024 {
            self.inner.recent_joins.retain(|_, at| now.duration_since(*at) <= RECENT_JOIN_WINDOW);
        }
        self.inner.recent_joins.insert(group.clone(), now);
        self.emit(Event::JoinedGroup { group: group.clone(), author, via });
    }

    pub(crate) fn forget_join(&self, group: &Jid) {
        self.inner.recent_joins.remove(group);
    }

    pub(crate) fn record_timings(&self, timings: SendTimings) {
        self.inner.timing_window.lock().push(timings, TIMING_WINDOW);
    }

    pub(crate) fn require_me(&self) -> Result<Jid> {
        let creds = self.inner.creds.read();
        match (&creds.me, &creds.account) {
            (Some(me), Some(_)) => Ok(me.clone()),
            _ => Err(Error::NotLoggedIn),
        }
    }

    pub(crate) async fn update_creds(&self, apply: impl FnOnce(&mut Creds)) -> Result<()> {
        let snapshot = {
            let mut creds = self.inner.creds.write();
            apply(&mut creds);
            creds.clone()
        };
        self.inner.store.save_creds(&snapshot).await
    }

    pub fn generate_tag(&self) -> String {
        let n = self.inner.counter.fetch_add(1, Ordering::Relaxed);
        format!("{}{}", self.inner.tag_prefix, n)
    }

    pub fn send_node(&self, node: &Node) -> Result<()> {
        let bytes = binary::encode(node);
        let link = self.inner.link.lock();
        let link = link.as_ref().ok_or(Error::NotConnected)?;
        link.outbound.send(bytes).map_err(|_| Error::NotConnected)
    }

    pub async fn query(&self, mut node: Node) -> Result<Node> {
        let id = match node.attr_str("id") {
            Some(id) => id.to_owned(),
            None => {
                let id = self.generate_tag();
                node.set_attr("id", id.clone());
                id
            }
        };
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().insert(id.clone(), tx);
        if let Err(err) = self.send_node(&node) {
            self.inner.pending.lock().remove(&id);
            return Err(err);
        }
        match tokio::time::timeout(self.inner.config.query_timeout, rx).await {
            Ok(Ok(response)) => match response.attr_str("type") {
                Some("error") => {
                    let (code, text) = response.error_code().unwrap_or((0, "unknown error".into()));
                    Err(Error::Server { code, text })
                }
                _ => Ok(response),
            },
            Ok(Err(_)) => Err(Error::NotConnected),
            Err(_) => {
                self.inner.pending.lock().remove(&id);
                Err(Error::Timeout)
            }
        }
    }

    pub(crate) fn end_link(&self, outcome: Outcome) {
        if let Some(link) = self.inner.link.lock().as_ref() {
            let _ = link.close.send(outcome);
        }
    }

    pub(crate) fn current_generation(&self) -> u64 {
        self.inner.generation.load(Ordering::Acquire)
    }

    fn dispatch(&self, node: Node) {
        if self.end_on_terminal(&node) {
            return;
        }
        if node.tag == "success" {
            let generation = self.current_generation();
            let client = self.clone();
            tokio::spawn(async move {
                let login = async {
                    if let Err(err) = client.on_success(&node, generation).await {
                        tracing::warn!(%err, "failed to finish the login");
                    }
                };
                client.contained("the login", login).await;
            });
            return;
        }
        let answer =
            node.tag == "call" || (node.tag == "iq" && matches!(node.attr_str("type"), Some("result" | "error")));
        if answer {
            let waiter = node.attr_str("id").and_then(|id| self.inner.pending.lock().remove(id));
            if let Some(waiter) = waiter {
                let _ = waiter.send(node);
                return;
            }
        }
        if node.tag == "message" {
            let lane = node
                .get("from")
                .map(|from| match from {
                    binary::Value::Jid(jid) => fnv(jid.user.as_bytes()),
                    binary::Value::Str(s) => fnv(s.as_bytes()),
                })
                .unwrap_or_default();
            let index = (lane as usize) % self.inner.lanes.len();
            let _ = self.inner.lanes[index].send(node);
            return;
        }
        let client = self.clone();
        tokio::spawn(async move { client.contained("a stanza handler", client.handle_node(node)).await });
    }

    pub(crate) async fn contained(&self, what: &str, work: impl std::future::Future<Output = ()>) {
        use futures_util::FutureExt;
        if let Err(panic) = std::panic::AssertUnwindSafe(work).catch_unwind().await {
            guard::Counters::bump(&self.inner.counters.panics);
            let detail = panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            tracing::error!(what, detail, "a handler panicked; the session keeps running");
        }
    }

    pub(crate) fn remember_sent(&self, id: &str, message: SentMessage) {
        self.inner.sent.lock().insert(id.to_owned(), message);
    }

    pub(crate) fn sent_message(&self, id: &str) -> Option<SentMessage> {
        self.inner.sent.lock().get(id)
    }

    pub(crate) fn bump_retry(&self, key: &str) -> u32 {
        let mut retries = self.inner.retries.lock();
        if retries.len() > 4096 {
            retries.clear();
        }
        let count = retries.entry(key.to_owned()).or_insert(0);
        *count += 1;
        *count
    }

    pub async fn wa_version(&self) -> [u32; 3] {
        if let Some(version) = self.inner.config.version {
            return version;
        }
        if let Some(version) = *self.inner.version.read() {
            return version;
        }
        let stored = self.inner.creds.read().wa_version;
        if let Some(version) = stored {
            *self.inner.version.write() = Some(version);
            let client = self.clone();
            tokio::spawn(async move { client.refresh_version().await });
            return version;
        }
        self.refresh_version().await.unwrap_or_else(|| {
            tracing::debug!("could not fetch the latest whatsapp web version, using the bundled one");
            DEFAULT_VERSION
        })
    }

    async fn refresh_version(&self) -> Option<[u32; 3]> {
        let latest = fetch_latest_version(&self.inner.http).await.ok()?;
        *self.inner.version.write() = Some(latest);
        if self.inner.creds.read().wa_version != Some(latest) {
            if let Err(err) = self.update_creds(|creds| creds.wa_version = Some(latest)).await {
                tracing::debug!(%err, "could not persist the whatsapp web version");
            }
        }
        Some(latest)
    }

    pub(crate) async fn forget_version(&self) {
        *self.inner.version.write() = None;
        let _ = self.update_creds(|creds| creds.wa_version = None).await;
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

pub async fn fetch_latest_version(http: &reqwest::Client) -> Result<[u32; 3]> {
    let body = http
        .get("https://web.whatsapp.com/sw.js")
        .header("sec-fetch-site", "none")
        .header(
            "user-agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .timeout(Duration::from_secs(8))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let start = body.find("client_revision").ok_or_else(|| Error::other("client_revision not found"))?;
    let digits: String = body[start + "client_revision".len()..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    let revision = digits.parse().map_err(|_| Error::other("client_revision is not a number"))?;
    Ok([2, 3000, revision])
}
