use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use dashmap::DashMap;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use soblendwa::interactive::{Interactive, NativeButton, Section};
use soblendwa::store::{
    import_baileys, Backend, BatchedStore, Creds, KeyKind, KeyWrite, MongoStore, PostgresStore, RedisStore, SqliteStore,
};
use soblendwa::{
    ChatPresence, ChatStore, Client, Error, Event, EventStream, FileStore, Jid, MediaKind, MediaOptions, MemoryStore,
    Message, MessageEvent, Platform, Proxy, Result, Store,
};
use soblendwa::{LinkPreview, LinkPreviewConfig, PreviewChoice, SendLimit};

mod dispatch;
mod ops;
mod order;
mod zapo;

pub use async_trait::async_trait;
pub use dispatch::METHODS;
pub use order::Turn;
pub use soblendwa::testing::FakeWhatsApp;
pub use soblendwa::Result as Outcome;
pub use zapo::{import_zapo, rollback_zapo, ZapoSource};

pub trait Describe {
    fn describe(&self) -> String;
}

impl Describe for Error {
    fn describe(&self) -> String {
        format!("[{}] {self}", self.code())
    }
}

impl Describe for serde_json::Error {
    fn describe(&self) -> String {
        format!("[INVALID_ARGUMENT] {self}")
    }
}

static SHARED_STORES: LazyLock<tokio::sync::Mutex<std::collections::HashMap<String, Arc<dyn Store>>>> =
    LazyLock::new(Default::default);

async fn shared_store(options: &StoreOptions) -> Result<Arc<dyn Store>> {
    let key = serde_json::to_string(options)?;
    let mut stores = SHARED_STORES.lock().await;
    if let Some(store) = stores.get(&key) {
        return Ok(store.clone());
    }
    let store = open_store(options).await?;
    stores.insert(key, store.clone());
    Ok(store)
}

pub fn fake_link(fake: &FakeWhatsApp, phone: &str) -> FakeLink {
    FakeLink {
        url: fake.url(),
        media_endpoint: fake.media_endpoint(),
        trust_root: fake.trust_root(),
        creds: fake.account(phone),
    }
}

const RECENT_MESSAGES: usize = 4096;

pub(crate) static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("soblendwa")
        .build()
        .expect("SoblendWA could not start its async runtime")
});

pub fn run<T, F>(work: F) -> impl Future<Output = Result<T>> + Send + 'static
where
    T: Send + 'static,
    F: Future<Output = Result<T>> + Send + 'static,
{
    let task = RUNTIME.spawn(work);
    async move { task.await.map_err(|err| Error::Other(format!("background task failed: {err}")))? }
}

pub async fn in_turn<T>(turn: Option<Turn>, work: impl Future<Output = T>) -> T {
    if let Some(turn) = &turn {
        turn.ready().await;
    }
    let out = work.await;
    drop(turn);
    out
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StoreOptions {
    Memory,
    File { path: String },
    Sqlite { path: String, account: Option<String> },
    Postgres { url: String, account: Option<String> },
    Redis { url: String, account: Option<String> },
    Mongo { url: String, database: Option<String>, account: Option<String> },
}

impl Default for StoreOptions {
    fn default() -> Self {
        StoreOptions::File { path: "session".into() }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub store: StoreOptions,
    pub push_name: Option<String>,
    pub device_name: Option<String>,
    pub platform: Option<String>,
    pub country_code: Option<String>,
    pub online_on_connect: Option<bool>,
    pub auto_reconnect: Option<bool>,
    pub proxy: Option<String>,
    pub chat_store: Option<ChatStoreOptions>,
    pub link_previews: Option<LinkPreviewOptions>,
    pub server: Option<ServerOverride>,
    pub media_store: Option<StoreOptions>,
    pub retry_deadline_secs: Option<u64>,
    pub send_limit: Option<SendLimitOptions>,
    #[serde(skip)]
    pub fake: Option<FakeLink>,
    #[serde(skip)]
    pub custom_store: Option<CustomStore>,
}

pub struct EventText {
    pub kind: &'static str,
    pub qr: Option<String>,
    pub json: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SendLimitOptions {
    Enabled(bool),
    Custom(SendLimitValues),
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SendLimitValues {
    pub per_minute: Option<u32>,
    pub burst: Option<u32>,
    pub per_chat_per_minute: Option<u32>,
    pub per_chat_burst: Option<u32>,
}

impl SendLimitOptions {
    fn limit(&self) -> Option<SendLimit> {
        let base = SendLimit::default();
        match self {
            SendLimitOptions::Enabled(false) => None,
            SendLimitOptions::Enabled(true) => Some(base),
            SendLimitOptions::Custom(values) => Some(SendLimit {
                per_minute: values.per_minute.unwrap_or(base.per_minute),
                burst: values.burst.unwrap_or(base.burst),
                per_chat_per_minute: values.per_chat_per_minute.unwrap_or(base.per_chat_per_minute),
                per_chat_burst: values.per_chat_burst.unwrap_or(base.per_chat_burst),
            }),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LinkPreviewOptions {
    pub enabled: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub upload_hq_thumbnail: Option<bool>,
    pub allow_private_hosts: Option<bool>,
    pub max_html_bytes: Option<usize>,
    pub max_image_bytes: Option<usize>,
    pub user_agent: Option<String>,
    pub cache_ttl_secs: Option<u64>,
}

impl LinkPreviewOptions {
    fn config(&self) -> LinkPreviewConfig {
        let mut config = LinkPreviewConfig::default();
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(timeout) = self.timeout_ms {
            config.timeout = Duration::from_millis(timeout);
        }
        if let Some(upload) = self.upload_hq_thumbnail {
            config.upload_hq_thumbnail = upload;
        }
        if let Some(allow) = self.allow_private_hosts {
            config.allow_private_hosts = allow;
        }
        if let Some(limit) = self.max_html_bytes {
            config.max_html_bytes = limit;
        }
        if let Some(limit) = self.max_image_bytes {
            config.max_image_bytes = limit;
        }
        if let Some(agent) = &self.user_agent {
            config.user_agent = agent.clone();
        }
        if let Some(ttl) = self.cache_ttl_secs {
            config.cache_ttl = (ttl > 0).then(|| Duration::from_secs(ttl));
        }
        config
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerOverride {
    pub ws_url: Option<String>,
    pub noise_root_hex: Option<String>,
    pub media_endpoint: Option<String>,
}

#[async_trait]
pub trait StoreHandler: Send + Sync {
    async fn call(&self, request: String) -> Result<Option<String>>;
}

#[derive(Clone)]
pub struct CustomStore(pub Arc<dyn StoreHandler>);

impl std::fmt::Debug for CustomStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomStore")
    }
}

struct RemoteBackend(Arc<dyn StoreHandler>);

impl RemoteBackend {
    async fn call(&self, request: Value) -> Result<Option<String>> {
        self.0.call(request.to_string()).await
    }
}

#[async_trait]
impl Backend for RemoteBackend {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        match self.call(json!({ "op": "loadCreds" })).await? {
            Some(raw) => serde_json::from_str(&raw).map(Some).map_err(Error::from),
            None => Ok(None),
        }
    }

    async fn save_creds(&self, creds: &Creds) -> Result<()> {
        let data = serde_json::to_string(creds)?;
        self.call(json!({ "op": "saveCreds", "data": data })).await.map(drop)
    }

    async fn fetch(&self, kind: KeyKind, key: &str) -> Result<Option<Vec<u8>>> {
        match self.call(json!({ "op": "get", "kind": kind.as_str(), "key": key })).await? {
            Some(raw) => {
                STANDARD.decode(raw).map(Some).map_err(|_| Error::Store("the custom store returned bad bytes".into()))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, kind: KeyKind) -> Result<Vec<String>> {
        match self.call(json!({ "op": "keys", "kind": kind.as_str() })).await? {
            Some(raw) => serde_json::from_str(&raw).map_err(Error::from),
            None => Ok(Vec::new()),
        }
    }

    async fn apply(&self, writes: &[KeyWrite]) -> Result<()> {
        let writes: Vec<Value> = writes
            .iter()
            .map(|write| match write {
                KeyWrite::Set(kind, key, value) => {
                    json!({ "op": "set", "kind": kind.as_str(), "key": key, "value": STANDARD.encode(value) })
                }
                KeyWrite::Remove(kind, key) => json!({ "op": "remove", "kind": kind.as_str(), "key": key }),
                KeyWrite::Clear => json!({ "op": "clear" }),
            })
            .collect();
        self.call(json!({ "op": "apply", "writes": writes })).await.map(drop)
    }
}

pub fn store_error(detail: impl Into<String>) -> Error {
    Error::Store(detail.into())
}

pub fn preview_choice(value: &Value) -> Result<PreviewChoice> {
    let bytes = |key: &str| -> Result<Option<Vec<u8>>> {
        match value[key].as_str() {
            Some(raw) => STANDARD
                .decode(raw.trim())
                .map(Some)
                .map_err(|_| Error::Other(format!("preview \"{key}\" must be base64"))),
            None => Ok(None),
        }
    };
    let text = |key: &str| value[key].as_str().map(str::to_owned);
    Ok(match value {
        Value::Null => PreviewChoice::Auto,
        Value::Bool(true) => PreviewChoice::On,
        Value::Bool(false) => PreviewChoice::Off,
        Value::Object(_) => PreviewChoice::Custom(Box::new(LinkPreview {
            url: text("url").unwrap_or_default(),
            title: text("title"),
            description: text("description"),
            image_url: None,
            thumbnail: bytes("thumbnail")?,
            image: bytes("image")?,
            width: value["width"].as_u64().map(|v| v as u32),
            height: value["height"].as_u64().map(|v| v as u32),
            video: value["video"].as_bool().unwrap_or(false),
        })),
        _ => return Err(Error::Other("preview must be a boolean or an object".into())),
    })
}

#[derive(Debug, Clone)]
pub struct FakeLink {
    pub url: String,
    pub media_endpoint: String,
    pub trust_root: [u8; 32],
    pub creds: soblendwa::store::Creds,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatStoreOptions {
    pub path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ButtonSpec {
    Reply { id: String, text: String },
    Url { text: String, url: String },
    Copy { text: String, code: String },
    Call { text: String, phone: String },
    Select { title: String, sections: Vec<SectionSpec> },
}

#[derive(Deserialize, Debug, Clone)]
pub struct SectionSpec {
    pub title: String,
    pub rows: Vec<RowSpec>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RowSpec {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct InteractiveSpec {
    pub body: String,
    pub footer: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub buttons: Vec<ButtonSpec>,
    pub quoted_id: Option<String>,
}

fn sections(specs: &[SectionSpec]) -> Vec<Section> {
    specs
        .iter()
        .map(|spec| {
            spec.rows.iter().fold(Section::new(spec.title.clone()), |section, row| match &row.description {
                Some(description) => section.described_row(row.id.clone(), row.title.clone(), description.clone()),
                None => section.row(row.id.clone(), row.title.clone()),
            })
        })
        .collect()
}

impl InteractiveSpec {
    fn build(&self) -> Message {
        let mut builder = Interactive::new(self.body.clone());
        if let Some(footer) = &self.footer {
            builder = builder.footer(footer.clone());
        }
        if let Some(title) = &self.title {
            builder = builder.title(title.clone());
        }
        if let Some(subtitle) = &self.subtitle {
            builder = builder.subtitle(subtitle.clone());
        }
        for button in &self.buttons {
            builder = builder.button(match button {
                ButtonSpec::Reply { id, text } => NativeButton::QuickReply { id: id.clone(), text: text.clone() },
                ButtonSpec::Url { text, url } => NativeButton::Url { text: text.clone(), url: url.clone() },
                ButtonSpec::Copy { text, code } => NativeButton::Copy { text: text.clone(), code: code.clone() },
                ButtonSpec::Call { text, phone } => NativeButton::Call { text: text.clone(), phone: phone.clone() },
                ButtonSpec::Select { title, sections: specs } => {
                    NativeButton::Select { title: title.clone(), sections: sections(specs) }
                }
            });
        }
        builder.build()
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MediaInput {
    pub caption: Option<String>,
    pub mimetype: Option<String>,
    pub file_name: Option<String>,
    pub voice_note: bool,
    pub gif: bool,
    pub view_once: bool,
    pub seconds: Option<u32>,
    pub quoted_id: Option<String>,
    pub mentions: Vec<String>,
}

pub fn parse_options(raw: Option<&str>) -> Result<Options> {
    match raw.map(str::trim).filter(|r| !r.is_empty()) {
        Some(raw) => serde_json::from_str(raw).map_err(Error::from),
        None => Ok(Options::default()),
    }
}

pub fn parse_media(raw: Option<&str>) -> Result<MediaInput> {
    match raw.map(str::trim).filter(|r| !r.is_empty()) {
        Some(raw) => serde_json::from_str(raw).map_err(Error::from),
        None => Ok(MediaInput::default()),
    }
}

pub fn jid(text: &str) -> Result<Jid> {
    if text.contains('@') {
        return text.parse();
    }
    let phone = Jid::phone(text);
    if phone.user.is_empty() {
        return Err(Error::InvalidJid(text.to_owned()));
    }
    Ok(phone)
}

fn platform(name: &str) -> Result<Platform> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "chrome" => Platform::Chrome,
        "firefox" => Platform::Firefox,
        "safari" => Platform::Safari,
        "edge" => Platform::Edge,
        "opera" => Platform::Opera,
        "desktop" => Platform::Desktop,
        other => return Err(Error::Other(format!("unknown platform {other}"))),
    })
}

fn media_kind(name: &str) -> Result<MediaKind> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "image" => MediaKind::Image,
        "video" => MediaKind::Video,
        "audio" => MediaKind::Audio,
        "document" => MediaKind::Document,
        "sticker" => MediaKind::Sticker,
        other => return Err(Error::Media(format!("unknown media kind {other}"))),
    })
}

pub(crate) async fn open_store(options: &StoreOptions) -> Result<Arc<dyn Store>> {
    let account = |a: &Option<String>| a.clone().unwrap_or_else(|| "default".into());
    Ok(match options {
        StoreOptions::Memory => Arc::new(MemoryStore::new()),
        StoreOptions::File { path } => FileStore::open(path).await?,
        StoreOptions::Sqlite { path, account: a } => Arc::new(SqliteStore::open_account(path, &account(a)).await?),
        StoreOptions::Postgres { url, account: a } => Arc::new(PostgresStore::connect_account(url, &account(a)).await?),
        StoreOptions::Redis { url, account: a } => Arc::new(RedisStore::connect_account(url, &account(a)).await?),
        StoreOptions::Mongo { url, database, account: a } => {
            Arc::new(MongoStore::connect_account(url, database.as_deref().unwrap_or("soblendwa"), &account(a)).await?)
        }
    })
}

struct Shared {
    client: Client,
    events: tokio::sync::Mutex<EventStream>,
    recent: DashMap<String, Arc<MessageEvent>>,
    order: Mutex<VecDeque<String>>,
    turns: order::Turnstiles,
}

#[derive(Clone)]
pub struct Session {
    shared: Arc<Shared>,
}

impl Session {
    pub async fn open(options: Options) -> Result<Session> {
        let store: Arc<dyn Store> = match &options.custom_store {
            Some(custom) => Arc::new(BatchedStore::new(RemoteBackend(custom.0.clone()))),
            None => open_store(&options.store).await?,
        };
        if let Some(fake) = &options.fake {
            if !store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
                store.save_creds(&fake.creds).await?;
            }
        }
        let mut builder = Client::builder().store(store);
        if let Some(fake) = &options.fake {
            builder = builder
                .ws_url(fake.url.clone())
                .trust_root(fake.trust_root)
                .media_endpoint(fake.media_endpoint.clone())
                .version(soblendwa::DEFAULT_VERSION);
        }
        if let Some(server) = &options.server {
            if let Some(url) = &server.ws_url {
                builder = builder.ws_url(url.clone()).version(soblendwa::DEFAULT_VERSION);
            }
            if let Some(root) = &server.noise_root_hex {
                let root: [u8; 32] = hex::decode(root)
                    .ok()
                    .and_then(|raw| raw.try_into().ok())
                    .ok_or_else(|| Error::Other("server.noiseRootHex must be 32 bytes in hex".into()))?;
                builder = builder.trust_root(root);
            }
            if let Some(endpoint) = &server.media_endpoint {
                builder = builder.media_endpoint(endpoint.clone());
            }
        }
        if let Some(previews) = &options.link_previews {
            builder = builder.link_previews(previews.config());
        }
        if let Some(media) = &options.media_store {
            builder = builder.media_store(shared_store(media).await?);
        }
        if let Some(seconds) = options.retry_deadline_secs {
            builder = builder.retry_deadline(Duration::from_secs(seconds));
        }
        if let Some(limit) = &options.send_limit {
            builder = builder.send_limit(limit.limit());
        }
        if let Some(proxy) = &options.proxy {
            builder = builder.proxy(Proxy::parse(proxy)?);
        }
        if let Some(chats) = &options.chat_store {
            let limit = chats.limit.unwrap_or(500);
            let store = match &chats.path {
                Some(path) => ChatStore::open_with_limit(path, limit).await?,
                None => Arc::new(ChatStore::with_limit(limit)),
            };
            builder = builder.chat_store(store);
        }
        if let Some(name) = options.push_name {
            builder = builder.push_name(name);
        }
        if let Some(name) = options.device_name {
            builder = builder.device_name(name);
        }
        if let Some(name) = options.platform {
            builder = builder.platform(platform(&name)?);
        }
        if let Some(code) = options.country_code {
            builder = builder.country_code(code);
        }
        if let Some(online) = options.online_on_connect {
            builder = builder.online_on_connect(online);
        }
        if let Some(enabled) = options.auto_reconnect {
            builder = builder.auto_reconnect(enabled);
        }
        let (client, events) = builder.build().await?;
        Ok(Session {
            shared: Arc::new(Shared {
                client,
                events: tokio::sync::Mutex::new(events),
                recent: DashMap::new(),
                order: Mutex::new(VecDeque::with_capacity(RECENT_MESSAGES)),
                turns: order::Turnstiles::default(),
            }),
        })
    }

    pub fn turn(&self, method: &str, params: &Value) -> Option<Turn> {
        match method {
            "reply" | "react" => self.turn_for_message(params["messageId"].as_str()?),
            _ => order::chat_of(method, params).map(|chat| self.turn_for_chat(&chat)),
        }
    }

    pub fn turn_for_chat(&self, chat: &str) -> Turn {
        let key = jid(chat).map(|j| j.to_string()).unwrap_or_else(|_| chat.to_owned());
        self.shared.turns.take(key)
    }

    pub fn turn_for_message(&self, id: &str) -> Option<Turn> {
        let chat = self.recall(id).ok()?.info.chat.to_string();
        Some(self.shared.turns.take(chat))
    }

    pub fn client(&self) -> &Client {
        &self.shared.client
    }

    pub fn me(&self) -> Option<String> {
        self.client().me().map(|j| j.to_string())
    }

    pub fn is_connected(&self) -> bool {
        self.client().is_connected()
    }

    pub async fn connect(&self) -> Result<()> {
        self.client().connect().await
    }

    pub async fn disconnect(&self) {
        self.client().disconnect().await
    }

    pub async fn logout(&self) -> Result<()> {
        self.client().logout().await
    }

    pub async fn next_event(&self) -> Option<Value> {
        let event = self.shared.events.lock().await.recv().await?;
        let json = event.to_json();
        self.keep(event);
        Some(json)
    }

    pub async fn next_event_text(&self) -> Option<EventText> {
        let event = self.shared.events.lock().await.recv().await?;
        let text = EventText {
            kind: event.kind(),
            qr: match &event {
                Event::Qr(code) => Some(code.clone()),
                _ => None,
            },
            json: event.to_json_string(),
        };
        self.keep(event);
        Some(text)
    }

    pub async fn next_events_json(&self, max: usize) -> Option<String> {
        let mut stream = self.shared.events.lock().await;
        let first = stream.recv().await?;
        let mut batch = vec![first];
        while batch.len() < max {
            match stream.try_recv() {
                Ok(event) => batch.push(event),
                Err(_) => break,
            }
        }
        drop(stream);
        let mut out = String::with_capacity(batch.len() * 512 + 2);
        out.push('[');
        for (index, event) in batch.into_iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&event.to_json_string());
            self.keep(event);
        }
        out.push(']');
        Some(out)
    }

    fn keep(&self, event: Event) {
        if let Event::Message(message) = event {
            self.remember(Arc::from(message));
        }
    }

    fn remember(&self, message: Arc<MessageEvent>) {
        let id = message.info.id.clone();
        if self.shared.recent.insert(id.clone(), message).is_some() {
            return;
        }
        let mut order = self.shared.order.lock();
        order.push_back(id);
        while order.len() > RECENT_MESSAGES {
            if let Some(old) = order.pop_front() {
                self.shared.recent.remove(&old);
            }
        }
    }

    fn recall(&self, id: &str) -> Result<Arc<MessageEvent>> {
        if let Some(recent) = self.shared.recent.get(id) {
            return Ok(recent.clone());
        }
        self.client()
            .chats()
            .and_then(|chats| chats.find_message(id))
            .map(|stored| Arc::new(stored.to_event()))
            .ok_or_else(|| Error::MessageNotFound(id.to_owned()))
    }

    fn quoting(&self, message: Message, quoted: Option<&str>) -> Result<Message> {
        match quoted {
            Some(id) => Ok(message.reply_to(self.recall(id)?.as_ref())),
            None => Ok(message),
        }
    }

    fn mentioning(message: Message, mentions: &[String]) -> Result<Message> {
        if mentions.is_empty() {
            return Ok(message);
        }
        let people = mentions.iter().map(|m| jid(m)).collect::<Result<Vec<_>>>()?;
        Ok(message.mention(people.iter()))
    }

    pub async fn send_text_full(
        &self,
        chat: &str,
        text: &str,
        quoted: Option<&str>,
        preview: &Value,
        mentions: &[String],
    ) -> Result<String> {
        let message = Self::mentioning(self.quoting(Message::text(text), quoted)?, mentions)?;
        let options = soblendwa::SendOptions { link_preview: preview_choice(preview)?, ..Default::default() };
        self.client().send_message_with(&jid(chat)?, message, options).await
    }

    pub fn elapsed_ms(&self, message_id: &str) -> Option<f64> {
        self.shared.recent.get(message_id).map(|m| m.info.elapsed().as_secs_f64() * 1000.0)
    }

    pub async fn ping(&self) -> Result<f64> {
        Ok(self.client().ping().await?.as_secs_f64() * 1000.0)
    }

    pub async fn pair_code(&self, phone: &str) -> Result<String> {
        self.client().request_pair_code(phone).await
    }

    pub async fn send_text(&self, chat: &str, text: &str, quoted: Option<&str>) -> Result<String> {
        let message = self.quoting(Message::text(text), quoted)?;
        self.client().send_message(&jid(chat)?, message).await
    }

    pub async fn send_text_with_preview(&self, chat: &str, text: &str) -> Result<String> {
        self.client().send_text_with_preview(&jid(chat)?, text).await
    }

    pub async fn send_text_choosing_preview(
        &self,
        chat: &str,
        text: &str,
        quoted: Option<&str>,
        preview: &Value,
    ) -> Result<String> {
        let message = self.quoting(Message::text(text), quoted)?;
        let options = soblendwa::SendOptions { link_preview: preview_choice(preview)?, ..Default::default() };
        self.client().send_message_with(&jid(chat)?, message, options).await
    }

    pub async fn send_proto(&self, chat: &str, proto: &[u8]) -> Result<String> {
        let message = <Message as soblendwa::prost::Message>::decode(proto)?;
        self.client().send_message(&jid(chat)?, message).await
    }

    pub async fn reply(&self, message_id: &str, text: &str) -> Result<String> {
        let original = self.recall(message_id)?;
        self.client().reply(&original, text).await
    }

    pub async fn react(&self, message_id: &str, emoji: &str) -> Result<String> {
        let original = self.recall(message_id)?;
        self.client().react(&original, emoji).await
    }

    pub async fn send_media(&self, chat: &str, kind: &str, data: Vec<u8>, input: MediaInput) -> Result<String> {
        let options = MediaOptions {
            mimetype: input.mimetype,
            caption: input.caption,
            file_name: input.file_name,
            voice_note: input.voice_note,
            gif: input.gif,
            view_once: input.view_once,
            seconds: input.seconds,
            ..Default::default()
        };
        let message = self.client().prepare_media(&data, media_kind(kind)?, options).await?;
        let message = Self::mentioning(self.quoting(message, input.quoted_id.as_deref())?, &input.mentions)?;
        self.client().send_message(&jid(chat)?, message).await
    }

    pub async fn send_media_file(&self, chat: &str, kind: &str, path: &str, input: MediaInput) -> Result<String> {
        let options = MediaOptions {
            mimetype: input.mimetype,
            caption: input.caption,
            file_name: input.file_name,
            voice_note: input.voice_note,
            gif: input.gif,
            view_once: input.view_once,
            seconds: input.seconds,
            ..Default::default()
        };
        let message = self.client().prepare_media_file(path, media_kind(kind)?, options).await?;
        let message = Self::mentioning(self.quoting(message, input.quoted_id.as_deref())?, &input.mentions)?;
        self.client().send_message(&jid(chat)?, message).await
    }

    pub async fn download_to_file(&self, message_id: &str, path: &str) -> Result<u64> {
        let original = self.recall(message_id)?;
        self.client().download_event_to_file(&original, path).await
    }

    pub async fn send_interactive(&self, chat: &str, spec: InteractiveSpec) -> Result<String> {
        let message = self.quoting(spec.build(), spec.quoted_id.as_deref())?;
        self.client().send_message(&jid(chat)?, message).await
    }

    pub async fn send_buttons(
        &self,
        chat: &str,
        text: &str,
        footer: Option<&str>,
        buttons: Vec<(String, String)>,
    ) -> Result<String> {
        self.client().send_message(&jid(chat)?, Message::buttons(text, footer, buttons)).await
    }

    pub async fn send_list(
        &self,
        chat: &str,
        title: &str,
        text: &str,
        button_text: &str,
        footer: Option<&str>,
        specs: Vec<SectionSpec>,
    ) -> Result<String> {
        let message = Message::list(title, text, button_text, footer, &sections(&specs));
        self.client().send_message(&jid(chat)?, message).await
    }

    pub async fn modify_chat(&self, chat: &str, action: &str, seconds: Option<u64>) -> Result<()> {
        let chat = jid(chat)?;
        let client = self.client();
        match action {
            "archive" => client.archive_chat(&chat, true).await,
            "unarchive" => client.archive_chat(&chat, false).await,
            "pin" => client.pin_chat(&chat, true).await,
            "unpin" => client.pin_chat(&chat, false).await,
            "mute" => client.mute_chat(&chat, Some(std::time::Duration::from_secs(seconds.unwrap_or(8 * 3600)))).await,
            "unmute" => client.mute_chat(&chat, None).await,
            "read" => client.mark_chat_read(&chat, true).await,
            "unread" => client.mark_chat_read(&chat, false).await,
            "delete" => client.delete_chat(&chat).await,
            "clear" => client.clear_chat(&chat).await,
            other => Err(Error::Other(format!("unknown chat action {other}"))),
        }
    }

    pub async fn label(&self, chat: &str, label_id: &str, message_id: Option<&str>, labeled: bool) -> Result<()> {
        let chat = jid(chat)?;
        match message_id {
            Some(id) => self.client().label_message(&chat, id, label_id, labeled).await,
            None => self.client().label_chat(&chat, label_id, labeled).await,
        }
    }

    pub async fn edit_label(&self, id: &str, name: Option<&str>, color: i32) -> Result<()> {
        match name {
            Some(name) => self.client().edit_label(id, name, color).await,
            None => self.client().delete_label(id).await,
        }
    }

    pub async fn save_quick_reply(&self, shortcut: &str, text: &str) -> Result<String> {
        self.client().save_quick_reply(None, shortcut, text).await
    }

    pub async fn send_contacts(&self, chat: &str, cards: Vec<(String, String)>) -> Result<String> {
        let display = match cards.len() {
            1 => cards[0].0.clone(),
            n => format!("{n} contactos"),
        };
        let cards: Vec<(String, String)> =
            cards.into_iter().map(|(name, phone)| (name.clone(), Message::vcard(&name, &phone))).collect();
        self.client().send_message(&jid(chat)?, Message::contacts(display, cards)).await
    }

    pub async fn send_mention_all(&self, chat: &str, text: &str) -> Result<String> {
        self.client().send_message(&jid(chat)?, Message::text(text).mention_all()).await
    }

    pub async fn send_group_invite(&self, to: &str, group: &str, caption: Option<&str>) -> Result<String> {
        self.client().send_group_invite(&jid(to)?, &jid(group)?, caption).await
    }

    pub async fn set_member_label(&self, group: &str, label: &str) -> Result<String> {
        self.client().set_member_label(&jid(group)?, label).await
    }

    pub async fn mark_played(&self, message_id: &str) -> Result<()> {
        let original = self.recall(message_id)?;
        self.client().mark_played(&original).await
    }

    pub async fn account(&self, what: &str, jids: Vec<String>) -> Result<Value> {
        let client = self.client();
        Ok(match what {
            "bots" => Value::Array(
                client
                    .bot_list()
                    .await?
                    .into_iter()
                    .map(|bot| json!({ "jid": bot.jid.to_string(), "personaId": bot.persona_id }))
                    .collect(),
            ),
            "timelock" => {
                let lock = client.reachout_timelock().await?;
                json!({ "active": lock.active, "endsAt": lock.ends_at, "enforcement": lock.enforcement })
            }
            "messageCap" => client.new_chat_message_cap().await?,
            "disappearing" => {
                let targets = jids.iter().map(|j| jid(j)).collect::<Result<Vec<_>>>()?;
                Value::Array(
                    client
                        .disappearing_modes(&targets)
                        .await?
                        .into_iter()
                        .map(
                            |m| json!({ "jid": m.jid.to_string(), "seconds": m.duration.as_secs(), "setAt": m.set_at }),
                        )
                        .collect(),
                )
            }
            other => return Err(Error::Other(format!("unknown account query {other}"))),
        })
    }

    fn chat_store(&self) -> Result<&Arc<ChatStore>> {
        self.client().chats().ok_or_else(|| Error::Other("open the session with the chatStore option first".into()))
    }

    pub fn chats(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.chat_store()?.chats())?)
    }

    pub fn contacts(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.chat_store()?.contacts())?)
    }

    pub fn messages(&self, chat: &str, limit: usize, before: Option<i64>) -> Result<Value> {
        let store = self.chat_store()?;
        let page = store.messages_before(&jid(chat)?, before.unwrap_or(i64::MAX), limit);
        Ok(Value::Array(page.iter().map(|m| m.to_json()).collect()))
    }

    pub fn search(&self, text: &str, limit: usize) -> Result<Value> {
        Ok(Value::Array(self.chat_store()?.search(text, limit).iter().map(|m| m.to_json()).collect()))
    }

    pub async fn download(&self, message_id: &str) -> Result<Vec<u8>> {
        let original = self.recall(message_id)?;
        self.client().download_event(&original).await
    }

    pub async fn send_poll(&self, chat: &str, name: &str, options: Vec<String>, selectable: u32) -> Result<String> {
        self.client().send_message(&jid(chat)?, Message::poll(name, options, selectable)).await
    }

    pub async fn vote_poll(&self, chat: &str, poll_id: &str, options: Vec<String>) -> Result<String> {
        let picked: Vec<&str> = options.iter().map(String::as_str).collect();
        self.client().vote_poll(&jid(chat)?, poll_id, &picked).await
    }

    pub async fn edit(&self, chat: &str, message_id: &str, text: &str) -> Result<String> {
        self.client().edit_message(&jid(chat)?, message_id, Message::text(text)).await
    }

    pub async fn revoke(&self, chat: &str, message_id: &str) -> Result<String> {
        let key = match self.recall(message_id) {
            Ok(original) => original.key(),
            Err(_) => soblendwa::proto::MessageKey {
                remote_jid: Some(jid(chat)?.to_string()),
                from_me: Some(true),
                id: Some(message_id.to_owned()),
                participant: None,
            },
        };
        self.client().revoke_message(&jid(chat)?, key).await
    }

    pub async fn chat_presence(&self, chat: &str, state: &str) -> Result<()> {
        let state = match state {
            "composing" | "typing" => ChatPresence::Composing,
            "recording" => ChatPresence::Recording,
            "paused" => ChatPresence::Paused,
            other => return Err(Error::Other(format!("unknown chat presence {other}"))),
        };
        self.client().send_chat_presence(&jid(chat)?, state).await
    }

    pub async fn set_presence(&self, available: bool) -> Result<()> {
        self.client().set_presence(available).await
    }

    pub async fn group_info(&self, chat: &str) -> Result<Value> {
        Ok(serde_json::to_value(self.client().group_info(&jid(chat)?).await?)?)
    }

    pub async fn joined_groups(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.client().joined_groups().await?)?)
    }

    pub async fn on_whatsapp(&self, phones: Vec<String>) -> Result<Value> {
        let phones: Vec<&str> = phones.iter().map(String::as_str).collect();
        let found = self.client().on_whatsapp(&phones).await?;
        Ok(Value::Array(
            found
                .into_iter()
                .map(|entry| {
                    json!({
                        "query": entry.query,
                        "jid": entry.jid.to_string(),
                        "exists": entry.exists,
                        "lid": entry.lid.map(|l| l.to_string()),
                    })
                })
                .collect(),
        ))
    }

    pub async fn profile_picture(&self, who: &str, full_size: bool) -> Result<Option<String>> {
        self.client().profile_picture_url(&jid(who)?, full_size).await
    }

    pub async fn block(&self, who: &str, blocked: bool) -> Result<()> {
        let target = jid(who)?;
        if blocked {
            self.client().block(&target).await
        } else {
            self.client().unblock(&target).await
        }
    }
}

pub fn credits() -> String {
    soblendwa::credits()
}

pub async fn migrate_baileys(source: &str, target: StoreOptions) -> Result<Value> {
    let store = open_store(&target).await?;
    if store.load_creds().await?.is_some_and(|creds| creds.is_logged_in()) {
        return Err(Error::Other("the target store already holds a linked session".into()));
    }
    let report = import_baileys(source, store.as_ref()).await?;
    Ok(json!({
        "me": report.me.map(|j| j.to_string()),
        "sessions": report.sessions,
        "preKeys": report.pre_keys,
        "senderKeys": report.sender_keys,
        "appStateKeys": report.app_state_keys,
        "appStateVersions": report.app_state_versions,
        "lidMappings": report.lid_mappings,
        "skipped": report.skipped,
        "failed": report.failed.iter().map(|(file, err)| json!({ "file": file, "error": err })).collect::<Vec<_>>(),
    }))
}

pub fn parse_store(raw: Option<&str>) -> Result<StoreOptions> {
    match raw.map(str::trim).filter(|r| !r.is_empty()) {
        Some(raw) => serde_json::from_str(raw).map_err(Error::from),
        None => Ok(StoreOptions::default()),
    }
}

pub fn version() -> &'static str {
    soblendwa::VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_jids() {
        assert_eq!(jid("+52 1 55 1234 5678").unwrap().to_string(), "5215512345678@s.whatsapp.net");
        assert_eq!(jid("120363000000000001@g.us").unwrap().to_string(), "120363000000000001@g.us");
        assert!(jid("hola").is_err());
    }

    #[test]
    fn reads_options_from_json() {
        let options =
            parse_options(Some(r#"{"store":{"type":"sqlite","path":"bot.db"},"pushName":"Soblend"}"#)).unwrap();
        assert!(matches!(options.store, StoreOptions::Sqlite { ref path, .. } if path == "bot.db"));
        assert_eq!(options.push_name.as_deref(), Some("Soblend"));
        assert!(matches!(parse_options(None).unwrap().store, StoreOptions::File { .. }));
    }

    #[test]
    fn shared_runtime() {
        let session = RUNTIME
            .block_on(run(async { Session::open(Options { store: StoreOptions::Memory, ..Default::default() }).await }))
            .unwrap();
        assert!(session.me().is_none());
        assert!(!session.is_connected());
    }
}
