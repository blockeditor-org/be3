//! The app's network (`network.wit`), its only one on every platform: `EditorHost::fetch`
//! sends a request through the host and `EditorHost::open_url` opens a link.

use be_sdk_api::{
    Failure,
    guest::network as wit,
    ops::{guest::Stream, wire},
};
use serde::de::DeserializeOwned;

pub use be_sdk_api::network::{Header, Request, ResponseHead};

use crate::Asked;

type Body = Stream<Result<Vec<u8>, Failure>, wire::Fallible<wire::Bytes, wire::Plain>>;

/// A response `EditorHost::fetch` answered with: its status and headers, whatever the
/// status, and its body, read as it arrives (`chunk`) or whole (`bytes`, `text`, `json`).
/// Dropping it aborts the request.
pub struct Response {
    pub status: u16,
    pub headers: Vec<Header>,
    fetch: wit::Fetch,
    body: Option<Body>,
}

impl Response {
    pub(crate) fn new(head: ResponseHead, fetch: wit::Fetch) -> Self {
        Self {
            status: head.status,
            headers: head.headers,
            fetch,
            body: None,
        }
    }

    /// Whether the status is a success (2xx).
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The first value of the header `name`, compared without case.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    /// The body's next piece as it arrives, `None` once it is whole. A failure is the last
    /// piece: the rest did not arrive.
    pub async fn chunk(&mut self) -> Option<Result<Vec<u8>, Failure>> {
        let fetch = &self.fetch;
        self.body.get_or_insert_with(|| fetch.body()).recv().await
    }

    /// The rest of the body once it is whole, if the status is a success; otherwise a
    /// failure with the status and the body's text. An app that tells statuses apart (a
    /// 401 to sign in again) reads `status` first, and `chunk` reads any body.
    pub fn bytes(mut self) -> Asked<Result<Vec<u8>, Failure>> {
        Asked::from_future(async move {
            let mut body = Vec::new();
            while let Some(chunk) = self.chunk().await {
                match chunk {
                    Ok(chunk) => body.extend_from_slice(&chunk),
                    Err(failure) => return Some(Err(failure)),
                }
            }
            if !self.ok() {
                let text = String::from_utf8_lossy(&body);
                let status = self.status;
                return Some(Err(Failure::Failed(format!(
                    "HTTP {status}: {}",
                    text.trim()
                ))));
            }
            Some(Ok(body))
        })
    }

    /// The rest of the body as UTF-8 text, as `bytes` reads it.
    pub fn text(self) -> Asked<Result<String, Failure>> {
        self.bytes().map(|body| {
            String::from_utf8(body?)
                .map_err(|_| Failure::Failed("the response is not UTF-8 text".to_owned()))
        })
    }

    /// The rest of the body decoded as JSON, as `bytes` reads it.
    pub fn json<T: DeserializeOwned + 'static>(self) -> Asked<Result<T, Failure>> {
        self.bytes().map(|body| {
            serde_json::from_slice(&body?).map_err(|error| {
                Failure::Failed(format!("the response is not the JSON expected: {error}"))
            })
        })
    }
}
