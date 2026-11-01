use std::{error::Error, fmt};

use be_block::ContentError;
use be_protocol::ErrorCode;
use be_store::{Hash, StoreError};

pub mod clock;
pub mod connection;
pub mod contacts;
pub mod keys;
pub mod peer;
pub mod refs;
pub mod transfer;
mod transport;

pub use clock::Clock;
pub use connection::{Answer, Connection, Event, ask};
pub use contacts::{Contact, GrantOffer};
pub use keys::{EscrowWarning, KeyBook};
pub use peer::{
    Credentials, Drained, Peer, PeerConfig, Reconciled, Resolver, Saved, Sealed, Write,
};
pub use refs::{PendingCounts, RefsDb, UnsentBlock};

pub fn socket_url(server_url: &str) -> String {
    let base = match server_url.split_once("://") {
        Some(("http", host)) => format!("ws://{host}"),
        Some(("https", host)) => format!("wss://{host}"),
        _ => server_url.to_owned(),
    };
    format!("{}/api/be", base.trim_end_matches('/'))
}

pub fn http_url(server_url: &str) -> String {
    let base = match server_url.split_once("://") {
        Some(("ws", host)) => format!("http://{host}"),
        Some(("wss", host)) => format!("https://{host}"),
        _ => server_url.to_owned(),
    };
    format!("{}/api/be/http", base.trim_end_matches('/'))
}

static AUTHORIZATION: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

pub fn authorize(server_url: &str, header: Option<String>) {
    let origin = origin(server_url);
    let mut held = AUTHORIZATION.lock().unwrap();
    held.retain(|(known, _)| *known != origin);
    if let Some(header) = header.filter(|header| !header.trim().is_empty()) {
        held.push((origin, header.trim().to_owned()));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn authorization(url: &str) -> Option<String> {
    let origin = origin(url);
    AUTHORIZATION
        .lock()
        .unwrap()
        .iter()
        .find(|(known, _)| *known == origin)
        .map(|(_, header)| header.clone())
}

fn origin(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[derive(Debug)]
pub enum ClientError {
    Refused(ErrorCode, String),
    Disconnected(String),
    TimedOut(&'static str),
    MissingObject(Hash),
    Tampered(Hash),
    Content(ContentError),
    Store(StoreError),
    Encoding(String),
    Contended,
    Locked(uuid::Uuid),
    Unexpected,
    Incompatible { ours: String, theirs: String },
    NoBuild(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(code, message) => {
                write!(formatter, "the server refused ({code}): {message}")
            }
            Self::Disconnected(reason) => write!(formatter, "the connection is gone: {reason}"),
            Self::TimedOut(what) => write!(formatter, "{what} timed out"),
            Self::MissingObject(hash) => write!(formatter, "object {hash} is nowhere to be found"),
            Self::Tampered(hash) => {
                write!(
                    formatter,
                    "the server returned bytes that are not object {hash}"
                )
            }
            Self::Content(error) => write!(formatter, "{error}"),
            Self::Store(error) => write!(formatter, "{error}"),
            Self::Encoding(reason) => write!(formatter, "a request failed to encode: {reason}"),
            Self::Contended => {
                formatter.write_str("someone else kept changing the block at the same time")
            }
            Self::Locked(_) => formatter.write_str(
                "this block is locked: its key has not been shared with this account yet",
            ),
            Self::Unexpected => formatter.write_str("the server answered with the wrong message"),
            Self::Incompatible { ours, theirs } => write!(
                formatter,
                "this app is build {ours} and the server is build {theirs}; update them to the same build"
            ),
            Self::NoBuild(why) => write!(formatter, "this program does not know its build: {why}"),
        }
    }
}

impl Error for ClientError {}

impl ClientError {
    pub fn is_unreachable(&self) -> bool {
        matches!(self, Self::Disconnected(_) | Self::TimedOut(_))
    }
}

impl From<be_ops::OpError> for ClientError {
    fn from(error: be_ops::OpError) -> Self {
        match error {
            be_ops::OpError::Canceled => Self::Disconnected("the connection closed".into()),
            be_ops::OpError::Unsupported => Self::Unexpected,
            be_ops::OpError::Denied => {
                Self::Refused(ErrorCode::PermissionDenied, error.to_string())
            }
        }
    }
}

impl From<be_protocol::Error> for ClientError {
    fn from(error: be_protocol::Error) -> Self {
        Self::Refused(error.code, error.message)
    }
}

impl From<StoreError> for ClientError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<ContentError> for ClientError {
    fn from(error: ContentError) -> Self {
        Self::Content(error)
    }
}

#[cfg(test)]
mod tests;
