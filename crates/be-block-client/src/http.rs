//! Plain HTTP GET and PUT to any URL, such as a presigned S3 URL that moves an object's bytes
//! (guides/blocks/server.md). Natively each request takes a connection of its own (http or
//! https, through `HTTPS_PROXY` as the websocket does); in the browser it is a CORS `fetch` that
//! sends no cookies, since a presigned URL carries its own authority.

use std::fmt;

use crate::transport;

/// The longest answer to a PUT that is read; a store answers an upload with an empty body.
const PUT_ANSWER_BYTES: usize = 64 * 1024;

/// Why a request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The server answered 404.
    NotFound,
    /// The server refused the request with another status that is not 2xx or 5xx (an expired
    /// or wrong signature, a checksum that does not match), and what it said why.
    Refused { status: u16, message: String },
    /// The answer is longer than the limit it was read with.
    TooLarge { limit: usize },
    /// The URL or a header cannot be sent.
    Invalid(String),
    /// The server could not be reached, stopped answering, failed (5xx) or sent something that
    /// is not HTTP; the same request may succeed later.
    Failed(String),
}

impl Error {
    pub(crate) fn from_status(status: u16, message: String) -> Self {
        match status {
            404 => Self::NotFound,
            500..=599 => Self::Failed(match message.is_empty() {
                true => format!("the server answered {status}"),
                false => format!("the server answered {status}: {message}"),
            }),
            status => Self::Refused { status, message },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(formatter, "not found"),
            Self::Refused { status, message } if message.is_empty() => {
                write!(formatter, "the server refused the request ({status})")
            }
            Self::Refused { status, message } => {
                write!(formatter, "the server refused the request ({status}): {message}")
            }
            Self::TooLarge { limit } => {
                write!(formatter, "the answer is longer than {limit} bytes")
            }
            Self::Invalid(reason) | Self::Failed(reason) => write!(formatter, "{reason}"),
        }
    }
}

impl std::error::Error for Error {}

/// Reads the body at `url`, refusing one longer than `limit` bytes without reading it all.
pub async fn get(url: &str, limit: usize) -> Result<Vec<u8>, Error> {
    transport::request("GET", url, &[], &[], limit).await
}

/// Uploads `body` to `url` with `headers` (such as `x-amz-checksum-sha256`). The request's
/// `Content-Length` is always the body's length, and `Host`, `Content-Length`, `Connection` and
/// `Transfer-Encoding` in `headers` are ignored: the browser sets them itself.
pub async fn put(url: &str, headers: &[(String, String)], body: Vec<u8>) -> Result<(), Error> {
    transport::request("PUT", url, headers, &body, PUT_ANSWER_BYTES)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests;
