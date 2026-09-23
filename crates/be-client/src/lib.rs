use std::{error::Error, fmt};

use be_block::ContentError;
use be_protocol::ErrorCode;
use be_store::{Hash, StoreError};

pub mod connection;
pub mod live;
pub mod peer;
mod transport;

pub use connection::Connection;
pub use live::{Journaled, Live};
pub use peer::{Credentials, Peer, PeerConfig, Saved};

#[derive(Debug)]
pub enum ClientError {
    Refused(ErrorCode, String),
    Disconnected(String),
    MissingObject(Hash),
    Tampered(Hash),
    Content(ContentError),
    Store(StoreError),
    Unexpected,
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(code, message) => {
                write!(formatter, "the server refused ({code}): {message}")
            }
            Self::Disconnected(reason) => write!(formatter, "the connection is gone: {reason}"),
            Self::MissingObject(hash) => write!(formatter, "object {hash} is nowhere to be found"),
            Self::Tampered(hash) => {
                write!(
                    formatter,
                    "the server returned bytes that are not object {hash}"
                )
            }
            Self::Content(error) => write!(formatter, "{error}"),
            Self::Store(error) => write!(formatter, "{error}"),
            Self::Unexpected => formatter.write_str("the server answered with the wrong message"),
        }
    }
}

impl Error for ClientError {}

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
