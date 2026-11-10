use thiserror::Error;

use crate::jid::Jid;

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

    #[error("the group {0} needs an admin to approve new members; a join request was sent")]
    ApprovalRequired(Jid),

    #[error("the profile photo of {0} is private")]
    PrivatePhoto(Jid),

    #[error("message {0} is not among the recent messages")]
    MessageNotFound(String),

    #[error("{0} has no WhatsApp devices to send to; check that the number is on WhatsApp")]
    NoDevices(Jid),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

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

    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Server { code, .. } => Some(*code),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Error::WebSocket(_) => "CONNECTION_FAILED",
            Error::Io(_) => "IO_ERROR",
            Error::Http(_) => "HTTP_ERROR",
            Error::Proto(_) => "PROTOBUF_ERROR",
            Error::Json(_) => "INVALID_JSON",
            Error::Binary(_) => "MALFORMED_NODE",
            Error::Crypto(_) => "CRYPTO_ERROR",
            Error::Signal(_) => "SIGNAL_ERROR",
            Error::Handshake(_) => "HANDSHAKE_FAILED",
            Error::Server { code, .. } => match code {
                400 => "BAD_REQUEST",
                401 => "NOT_AUTHORIZED",
                403 => "FORBIDDEN",
                404 => "NOT_FOUND",
                405 => "NOT_ALLOWED",
                406 => "NOT_ACCEPTABLE",
                408 => "REQUEST_TIMEOUT",
                409 => "CONFLICT",
                410 => "GONE",
                429 => "RATE_OVERLIMIT",
                500 => "INTERNAL_SERVER_ERROR",
                501 => "NOT_IMPLEMENTED",
                503 => "SERVICE_UNAVAILABLE",
                _ => "SERVER_ERROR",
            },
            Error::Timeout => "TIMEOUT",
            Error::NotConnected => "NOT_CONNECTED",
            Error::NotLoggedIn => "NOT_LOGGED_IN",
            Error::InvalidJid(_) => "INVALID_JID",
            Error::Media(_) => "MEDIA_ERROR",
            Error::Store(_) => "STORE_ERROR",
            Error::MissingAppStateKey(_) => "MISSING_APP_STATE_KEY",
            Error::RateLimited { .. } => "RATE_LIMITED",
            Error::AccountBlocked(_) => "ACCOUNT_BLOCKED",
            Error::ApprovalRequired(_) => "APPROVAL_REQUIRED",
            Error::PrivatePhoto(_) => "PRIVATE_PHOTO",
            Error::MessageNotFound(_) => "MESSAGE_NOT_FOUND",
            Error::NoDevices(_) => "NO_DEVICES",
            Error::InvalidArgument(_) => "INVALID_ARGUMENT",
            Error::Other(_) => "OTHER",
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes() {
        let server = |code| Error::Server { code, text: String::new() };
        assert_eq!(server(403).code(), "FORBIDDEN");
        assert_eq!(server(404).status(), Some(404));
        assert_eq!(server(499).code(), "SERVER_ERROR");
        assert_eq!(Error::ApprovalRequired(Jid::group("1")).code(), "APPROVAL_REQUIRED");
        assert_eq!(Error::PrivatePhoto(Jid::phone("1")).code(), "PRIVATE_PHOTO");
        assert_eq!(Error::NoDevices(Jid::phone("1")).code(), "NO_DEVICES");
        assert_eq!(Error::Timeout.status(), None);
    }
}
