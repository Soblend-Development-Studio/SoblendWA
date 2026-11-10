use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::{json, Value};
use soblendwa_bridge::{ChatStoreOptions, Options, Session, StoreOptions, METHODS};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

mod metrics;

const EVENT_BUFFER: usize = 4096;
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub enum Storage {
    Memory,
    Directory(PathBuf),
    Sqlite(PathBuf),
    Mongo { url: String, database: String },
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub token: Option<String>,
    pub storage: Storage,
    pub webhook: Option<String>,
    pub proxy: Option<String>,
    pub push_name: Option<String>,
    pub chat_store: bool,
    pub fake: Option<Arc<soblendwa::testing::FakeWhatsApp>>,
    pub link_previews: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            token: None,
            storage: Storage::Directory(PathBuf::from("sessions")),
            fake: None,
            link_previews: true,
            webhook: None,
            proxy: None,
            push_name: None,
            chat_store: true,
        }
    }
}

pub struct Live {
    name: String,
    session: Session,
    events: broadcast::Sender<Arc<str>>,
    qr: Mutex<Option<String>>,
}

impl Live {
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<str>> {
        self.events.subscribe()
    }

    fn status(&self) -> Value {
        json!({
            "session": self.name,
            "me": self.session.me(),
            "connected": self.session.is_connected(),
            "qr": self.qr.lock().clone(),
        })
    }
}

pub struct Hub {
    config: ServerConfig,
    sessions: tokio::sync::Mutex<HashMap<String, Arc<Live>>>,
    http: reqwest::Client,
}

pub fn fake_phone(name: &str) -> String {
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    format!("52100{:08}", hash % 100_000_000)
}

fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Hub {
    pub fn new(config: ServerConfig) -> Arc<Hub> {
        Arc::new(Hub { config, sessions: tokio::sync::Mutex::new(HashMap::new()), http: reqwest::Client::new() })
    }

    fn options_for(&self, name: &str) -> Options {
        let (store, chats) = match &self.config.storage {
            Storage::Memory => (StoreOptions::Memory, None),
            Storage::Directory(root) => {
                let dir = root.join(name);
                let chats = dir.join("chats.json").to_string_lossy().into_owned();
                (StoreOptions::File { path: dir.to_string_lossy().into_owned() }, Some(chats))
            }
            Storage::Mongo { url, database } => (
                StoreOptions::Mongo { url: url.clone(), database: Some(database.clone()), account: Some(name.into()) },
                None,
            ),
            Storage::Sqlite(path) => {
                let chats = path.with_file_name(format!("{name}-chats.json")).to_string_lossy().into_owned();
                (
                    StoreOptions::Sqlite { path: path.to_string_lossy().into_owned(), account: Some(name.into()) },
                    Some(chats),
                )
            }
        };
        let fake = self.config.fake.as_ref().map(|fake| soblendwa_bridge::FakeLink {
            url: fake.url(),
            media_endpoint: fake.media_endpoint(),
            trust_root: fake.trust_root(),
            creds: fake.account(&fake_phone(name)),
        });
        let store = if fake.is_some() { StoreOptions::Memory } else { store };
        Options {
            store,
            fake,
            link_previews: Some(soblendwa_bridge::LinkPreviewOptions {
                enabled: Some(self.config.link_previews),
                ..Default::default()
            }),
            push_name: self.config.push_name.clone(),
            proxy: self.config.proxy.clone(),
            chat_store: self.config.chat_store.then_some(ChatStoreOptions { path: chats, limit: None }),
            ..Default::default()
        }
    }

    pub async fn open(self: &Arc<Self>, name: &str) -> Result<Arc<Live>, String> {
        if !valid_name(name) {
            return Err(format!("invalid session name \"{name}\": use letters, digits, - or _"));
        }
        if let Some(live) = self.sessions.lock().await.get(name) {
            return Ok(live.clone());
        }
        let session = Session::open(self.options_for(name)).await.map_err(|err| err.to_string())?;
        let live = {
            let mut sessions = self.sessions.lock().await;
            if let Some(winner) = sessions.get(name) {
                return Ok(winner.clone());
            }
            let (events, _) = broadcast::channel(EVENT_BUFFER);
            let live = Arc::new(Live { name: name.to_owned(), session, events, qr: Mutex::new(None) });
            sessions.insert(name.to_owned(), live.clone());
            live
        };
        tokio::spawn(pump(self.clone(), live.clone()));
        if let Err(err) = live.session.connect().await {
            self.sessions.lock().await.remove(name);
            return Err(err.to_string());
        }
        tracing::info!(session = name, "session opened");
        Ok(live)
    }

    pub async fn names(&self) -> Vec<String> {
        self.sessions.lock().await.keys().cloned().collect()
    }

    pub async fn close_all(&self) {
        let sessions: Vec<Arc<Live>> = self.sessions.lock().await.drain().map(|(_, live)| live).collect();
        for live in sessions {
            live.session.disconnect().await;
        }
    }

    fn authorized(&self, headers: &HeaderMap, query: &HashMap<String, String>) -> bool {
        let Some(token) = &self.config.token else { return true };
        let bearer = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
        let supplied = bearer.or_else(|| query.get("token").map(String::as_str));
        supplied.is_some_and(|given| same(given.as_bytes(), token.as_bytes()))
    }

    fn deliver(self: &Arc<Self>, frame: Arc<str>) {
        let Some(url) = self.config.webhook.clone() else { return };
        let hub = self.clone();
        tokio::spawn(async move {
            let mut request = hub
                .http
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .timeout(WEBHOOK_TIMEOUT)
                .body(frame.to_string());
            if let Some(token) = &hub.config.token {
                request = request.header(AUTHORIZATION, format!("Bearer {token}"));
            }
            match request.send().await.and_then(|r| r.error_for_status()) {
                Ok(_) => {}
                Err(err) => tracing::warn!(%err, "webhook delivery failed"),
            }
        });
    }
}

async fn pump(hub: Arc<Hub>, live: Arc<Live>) {
    let session = serde_json::to_string(&live.name).unwrap_or_default();
    while let Some(event) = live.session.next_event_text().await {
        match event.kind {
            "qr" => *live.qr.lock() = event.qr,
            "connected" | "pairSuccess" | "loggedOut" => *live.qr.lock() = None,
            _ => {}
        }
        let frame: Arc<str> = format!("{{\"session\":{session},\"event\":{}}}", event.json).into();
        let _ = live.events.send(frame.clone());
        hub.deliver(frame);
    }
}

fn failure(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": { "message": message.into(), "code": code } }))).into_response()
}

async fn info(State(hub): State<Arc<Hub>>) -> Json<Value> {
    Json(json!({
        "name": "SoblendWA server",
        "version": soblendwa_bridge::version(),
        "author": "Soblend Development Studio",
        "sessions": hub.names().await,
        "methods": METHODS,
    }))
}

async fn prometheus(
    State(hub): State<Arc<Hub>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if !hub.authorized(&headers, &query) {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "missing or wrong token");
    }
    let sessions: Vec<Arc<Live>> = hub.sessions.lock().await.values().cloned().collect();
    let mut stats: Vec<(String, soblendwa::Stats)> =
        sessions.iter().map(|live| (live.name.clone(), live.session.client().stats())).collect();
    stats.sort_by(|a, b| a.0.cmp(&b.0));
    let body = metrics::render(&stats, metrics::resident_bytes());
    ([(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], body).into_response()
}

async fn list_sessions(
    State(hub): State<Arc<Hub>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if !hub.authorized(&headers, &query) {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "missing or wrong token");
    }
    let sessions: Vec<Arc<Live>> = hub.sessions.lock().await.values().cloned().collect();
    Json(Value::Array(sessions.iter().map(|live| live.status()).collect())).into_response()
}

async fn status(
    State(hub): State<Arc<Hub>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if !hub.authorized(&headers, &query) {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "missing or wrong token");
    }
    match hub.open(&name).await {
        Ok(live) => Json(live.status()).into_response(),
        Err(err) => failure(StatusCode::BAD_REQUEST, "INVALID_SESSION", err),
    }
}

async fn rpc(
    State(hub): State<Arc<Hub>>,
    Path((name, method)): Path<(String, String)>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    body: Bytes,
) -> Response {
    if !hub.authorized(&headers, &query) {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "missing or wrong token");
    }
    let params: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(params) => params,
            Err(err) => {
                return failure(StatusCode::BAD_REQUEST, "INVALID_JSON", format!("body is not valid json: {err}"))
            }
        }
    };
    let live = match hub.open(&name).await {
        Ok(live) => live,
        Err(err) => return failure(StatusCode::BAD_REQUEST, "INVALID_SESSION", err),
    };
    match live.session.call(&method, &params).await {
        Ok(result) => Json(json!({ "result": result })).into_response(),
        Err(err) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "message": err.to_string(), "code": err.code(), "status": err.status() } })),
        )
            .into_response(),
    }
}

async fn upgrade(
    State(hub): State<Arc<Hub>>,
    name: Option<Path<String>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    socket: WebSocketUpgrade,
) -> Response {
    if !hub.authorized(&headers, &query) {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "missing or wrong token");
    }
    let name = name.map(|Path(name)| name).unwrap_or_else(|| "default".into());
    match hub.open(&name).await {
        Ok(live) => socket.on_upgrade(move |socket| client(live, socket)).into_response(),
        Err(err) => failure(StatusCode::BAD_REQUEST, "INVALID_SESSION", err),
    }
}

async fn client(live: Arc<Live>, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (outgoing, mut queued) = mpsc::unbounded_channel::<String>();
    let mut hello = live.status();
    hello["type"] = json!("hello");
    hello["methods"] = json!(METHODS);
    let _ = outgoing.send(hello.to_string());
    if let Some(code) = live.qr.lock().clone() {
        let _ = outgoing.send(json!({ "session": live.name, "event": { "type": "qr", "code": code } }).to_string());
    }
    let mut events = live.subscribe();
    let writer = tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                next = queued.recv() => match next {
                    Some(text) => text,
                    None => break,
                },
                event = events.recv() => match event {
                    Ok(frame) => frame.to_string(),
                    Err(broadcast::error::RecvError::Lagged(missed)) => {
                        json!({ "type": "lagged", "missed": missed }).to_string()
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            };
            if sink.send(WsMessage::Text(frame.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        let text = match message {
            WsMessage::Text(text) => text.to_string(),
            WsMessage::Binary(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            WsMessage::Close(_) => break,
            _ => continue,
        };
        let request: Value = match serde_json::from_str(&text) {
            Ok(request) => request,
            Err(err) => {
                let reply = json!({ "id": null, "error": { "message": format!("invalid json: {err}"), "code": "INVALID_JSON" } });
                let _ = outgoing.send(reply.to_string());
                continue;
            }
        };
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default().to_owned();
        let params = match &request["params"] {
            Value::Null => json!({}),
            params => params.clone(),
        };
        let turn = live.session.turn(&method, &params);
        let live = live.clone();
        let outgoing = outgoing.clone();
        tokio::spawn(async move {
            let reply = match soblendwa_bridge::in_turn(turn, live.session.begin(&method, params)).await {
                Ok(result) => json!({ "id": id, "result": result }),
                Err(err) => {
                    json!({ "id": id, "error": { "message": err.to_string(), "code": err.code(), "status": err.status() } })
                }
            };
            let _ = outgoing.send(reply.to_string());
        });
    }
    writer.abort();
}

pub fn router(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/", get(info))
        .route("/sessions", get(list_sessions))
        .route("/metrics", get(prometheus))
        .route("/ws", get(upgrade))
        .route("/ws/{session}", get(upgrade))
        .route("/api/{session}", get(status))
        .route("/api/{session}/{method}", post(rpc))
        .with_state(hub)
}

pub async fn serve(
    listener: TcpListener,
    hub: Arc<Hub>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, router(hub.clone())).with_graceful_shutdown(shutdown).await?;
    hub.close_all().await;
    Ok(())
}
