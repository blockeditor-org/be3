use std::{error::Error, fmt};

use be_block_content::ContentError;
use be_block_protocol::ErrorCode;
use be_block_store::{Hash, StoreError};

pub mod clock;
pub mod connection;
pub mod contacts;
pub mod http;
pub mod keys;
pub mod peer;
pub mod refs;
pub mod transfer;
mod transport;

pub use clock::Clock;
pub use connection::{Answer, Connection, Credentials, Event, ask};
pub use contacts::{Contact, GrantOffer};
pub use keys::{EscrowWarning, KeyBook};
pub use peer::account_block::AccountRecords;
pub use peer::hosted::{Admitted, Admitting, Joining};
pub use peer::log::{Appended, ExportedEntry, LogRead, LoggedEntry};
pub use peer::{
    BlockRead, Drained, Listed, MAX_NOTIFICATION_AGE_MILLISECONDS, MAX_NOTIFICATION_TEXT,
    Notification, OpenedNotification, Peer, PeerConfig, PendingRotation, Reconciled, Resolver,
    Saved, Sealed, SealsJournal, Signer, Write, by_app,
};
pub use refs::{
    Aside, KeptChange, KeptEdit, KeptLogHead, KeptSubmission, ListedBlock, OpenEdits,
    PendingCounts, RefsDb, SetAside, UnsentBlock,
};
#[cfg(not(target_arch = "wasm32"))]
pub use transport::tls_config;

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

/// Where one-shot calls are posted (`call_once`).
pub fn call_url(server_url: &str) -> String {
    let base = match server_url.split_once("://") {
        Some(("ws", host)) => format!("http://{host}"),
        Some(("wss", host)) => format!("https://{host}"),
        _ => server_url.to_owned(),
    };
    format!("{}/api/be/call", base.trim_end_matches('/'))
}

/// Makes one call that needs no sign-in (beginning or polling a browser sign-in, or reading an
/// invitation) as a one-shot post: a greeting and the call, answered and closed. Signing in
/// holds no connection open while a person approves it in their browser.
pub async fn call_once<T, F>(
    server_url: &str,
    start: impl FnOnce(&be_wasm_ops::net::Peer) -> F,
) -> Result<T, ClientError>
where
    F: Future<Output = Result<Result<T, be_block_protocol::Error>, be_wasm_ops::OpError>>,
{
    let ours = be_lib_build_id::current().map_err(ClientError::NoBuild)?;
    let (sending, sent) = std::sync::mpsc::channel();
    let peer = be_wasm_ops::net::Peer::new(be_wasm_ops::Side::Guest, move |frame| {
        let _ = sending.send(frame);
    });
    let pending = start(&peer);
    let mut body = transport::framed(&be_block_protocol::hello(
        ours,
        &be_block_protocol::Greeting::default(),
    ));
    while let Ok(frame) = sent.try_recv() {
        body.extend_from_slice(&transport::framed(&frame));
    }
    let answer = transport::post_once(&call_url(server_url), &body)
        .await
        .map_err(ClientError::Disconnected)?;
    let mut answer = answer.into_iter();
    let greeting = answer.next();
    let theirs = greeting
        .as_deref()
        .and_then(be_block_protocol::read_hello)
        .ok_or_else(|| ClientError::Incompatible {
            ours: ours.to_owned(),
            theirs: "a build that does not greet".to_owned(),
        })?
        .build;
    if !be_lib_build_id::compatible(ours, &theirs) {
        return Err(ClientError::Incompatible {
            ours: ours.to_owned(),
            theirs,
        });
    }
    for frame in answer {
        peer.receive(&frame)
            .map_err(|error| ClientError::Disconnected(error.to_string()))?;
    }
    peer.close();
    connection::ask(pending).await
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
    /// The server served access records for the block that do not check back to the root
    /// this device pinned (guides/blocks/keys.md).
    Forged(uuid::Uuid),
    /// The server served the block's access log (or, for the organization's id, the
    /// organization's) short of what this device checked of it before: it holds back history,
    /// so nothing is taken from it (guides/blocks/keys.md, "Epochs name their heads").
    Withheld(uuid::Uuid),
    /// The server served a head of the block's signed log that does not extend the head this
    /// device verified before: it cut, rewrote or forked the log (plans/signed_logs.md), so
    /// nothing of it is taken.
    LogForked(uuid::Uuid),
    /// What would be sent of the block is sealed under a key epoch its access log shows
    /// outdated, which someone who lost access may hold: not sent (guides/blocks/keys.md, "Epochs
    /// name their heads").
    Outdated(uuid::Uuid),
    /// This device's edits to the block were discarded and it may not edit it: it keeps none
    /// (`Refs::keep_open_edits`).
    Discarded(uuid::Uuid),
    Unexpected,
    Incompatible {
        ours: String,
        theirs: String,
    },
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
            Self::Forged(_) => {
                formatter.write_str("the server served access records its members did not sign")
            }
            Self::Outdated(_) => formatter.write_str(
                "this edit was locked offline with a key someone who lost access may hold, so it was not sent",
            ),
            Self::LogForked(_) => formatter.write_str(
                "the server served this block's log without entries this device saw in it before",
            ),
            Self::Withheld(_) => formatter.write_str(
                "the server served less of this block's access history than this device saw before",
            ),
            Self::Discarded(_) => formatter.write_str(
                "this device's edits to this block were discarded, and it may not edit it",
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

impl From<be_wasm_ops::OpError> for ClientError {
    fn from(error: be_wasm_ops::OpError) -> Self {
        match error {
            be_wasm_ops::OpError::Canceled => Self::Disconnected("the connection closed".into()),
            be_wasm_ops::OpError::Unsupported | be_wasm_ops::OpError::Unanswered => {
                Self::Unexpected
            }
            be_wasm_ops::OpError::Denied => {
                Self::Refused(ErrorCode::PermissionDenied, error.to_string())
            }
        }
    }
}

impl From<be_block_protocol::Error> for ClientError {
    fn from(error: be_block_protocol::Error) -> Self {
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
