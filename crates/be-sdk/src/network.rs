//! The app's network (`network.wit`), its only one on every platform: `EditorHost::fetch`
//! sends a request through the host and `EditorHost::open_url` opens a link.

use be_sdk_api::{
    Failure,
    guest::network as wit,
    ops::{OpError, guest::Stream, wire},
};
use serde::de::DeserializeOwned;

pub use be_sdk_api::network::{Header, Request, ResponseHead};

use crate::Asked;

type Body = Stream<Result<Vec<u8>, Failure>, wire::Fallible<wire::Bytes, wire::Plain>>;

/// A response `EditorHost::fetch` answered with: its status and headers (`head`), whatever
/// the status, and its body, read as it arrives (`chunk`) or whole (`bytes`, `text`,
/// `json`), also whatever the status, so an API's error body reads like any other.
/// `success` turns a status that is not a success into a failure. Dropping the response
/// aborts the request.
pub struct Response {
    pub head: ResponseHead,
    fetch: wit::Fetch,
    body: Option<Body>,
    ended: bool,
}

/// One step of reading a body.
enum Piece {
    Chunk(Vec<u8>),
    Whole,
    Failed(Failure),
    Canceled,
}

impl Response {
    pub(crate) fn new(head: ResponseHead, fetch: wit::Fetch) -> Self {
        Self {
            head,
            fetch,
            body: None,
            ended: false,
        }
    }

    /// The response itself if its status is a success (2xx), and otherwise a failure naming
    /// the status. Chain it before reading a body the app only wants on success:
    /// `host.fetch(request).map(|answer| answer.and_then(Response::success))
    /// .and_then(Response::json::<T>)`.
    pub fn success(self) -> Result<Self, Failure> {
        match self.head.ok() {
            true => Ok(self),
            false => Err(Failure::Failed(format!("HTTP {}", self.head.status))),
        }
    }

    async fn piece(&mut self) -> Piece {
        if self.ended {
            return Piece::Whole;
        }
        let fetch = &self.fetch;
        let body = self.body.get_or_insert_with(|| fetch.body());
        let piece = match body.recv().await {
            Some(Ok(chunk)) => return Piece::Chunk(chunk),
            Some(Err(failure)) => Piece::Failed(failure),
            None => match body.failed() {
                None => Piece::Whole,
                Some(OpError::Canceled) => Piece::Canceled,
                Some(error) => Piece::Failed(Failure::from(error)),
            },
        };
        self.ended = true;
        piece
    }

    /// The body's next piece as it arrives, `None` once it is whole. A failure is the last
    /// piece: the rest did not arrive, or the host stopped sending it.
    pub async fn chunk(&mut self) -> Option<Result<Vec<u8>, Failure>> {
        match self.piece().await {
            Piece::Chunk(chunk) => Some(Ok(chunk)),
            Piece::Whole => None,
            Piece::Failed(failure) => Some(Err(failure)),
            Piece::Canceled => Some(Err(Failure::from(OpError::Canceled))),
        }
    }

    /// The rest of the body once it is whole, whatever the status; answers nothing when the
    /// host canceled it, as any canceled call does.
    pub fn bytes(mut self) -> Asked<Result<Vec<u8>, Failure>> {
        Asked::from_future(async move {
            let mut body = Vec::new();
            loop {
                match self.piece().await {
                    Piece::Chunk(chunk) => body.extend_from_slice(&chunk),
                    Piece::Whole => return Some(Ok(body)),
                    Piece::Failed(failure) => return Some(Err(failure)),
                    Piece::Canceled => return None,
                }
            }
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
