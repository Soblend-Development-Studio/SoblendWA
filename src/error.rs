use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("websocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("protobuf decode error: {0}")]
    Proto(#[from] prost::DecodeError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("malformed binary node: {0}")]
    Binary(String),

    #[error("crypto failure: {0}")]
    Crypto(&'static str),

    #[error("signal protocol: {0}")]
    Signal(String),

    #[error("noise handshake failed: {0}")]
    Handshake(String),

    #[error("server answered with error {code}: {text}")]
    Server { code: u16, text: String },

    #[error("the request timed out")]
    Timeout,

    #[error("the client is not connected")]
    NotConnected,

    #[error("the client is not logged in")]
    NotLoggedIn,

    #[error("invalid jid: {0}")]
    InvalidJid(String),

    #[error("media error: {0}")]
    Media(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("missing app state key: {0}")]
    MissingAppStateKey(String),

    #[error("send limit reached for {chat}, try again in {retry_after:?}")]
    RateLimited { chat: String, retry_after: std::time::Duration },

    #[error("the account is blocked by whatsapp: {0}")]
    AccountBlocked(String),

    #[error("{0}")]
    Other(String),
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(Box::new(err))
    }
}

impl Error {
    pub(crate) fn binary(msg: impl Into<String>) -> Self {
        Error::Binary(msg.into())
    }

    pub(crate) fn signal(msg: impl Into<String>) -> Self {
        Error::Signal(msg.into())
    }

    pub(crate) fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
