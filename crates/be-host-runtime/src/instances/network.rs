//! An app's `network` interface: its fetches and the links it opens.
//!
//! A fetch is the app's `fetch` resource. The host sends its request with
//! `be_host_base::platform::http::send`, which reads the response on a thread of its own
//! natively (the browser's `fetch` on the web) and posts each part here as it arrives. The
//! head answers `response`; each body chunk waits here until the app's `body` stream has
//! credit for it, and only then lets the reading go one chunk further (`Flow::release`), so
//! a body of any length crosses with no more than a few chunks held on the host.

use std::collections::VecDeque;

use be_lib_stable_hash::HashMap;
use be_sdk_api::{
    EditorInstanceId, Failure,
    embed::network::Call,
    network::{Header, ResponseHead},
    ops::{OpError, OpId, Outcome, Reply, wire},
};
use be_host_base::platform::{
    self,
    http::{self, Event, Flow},
};

use super::{Instances, allowed};

type BodyItem = Result<Vec<u8>, Failure>;
type BodyEncoding = wire::Fallible<wire::Bytes, wire::Plain>;

/// What the request's reading posts: its fetch's resource, the epoch it was sent in, and
/// what arrived.
pub(super) struct Arrived {
    epoch: u64,
    fetch: OpId,
    event: Event,
}

#[derive(Default)]
pub(super) struct Fetches {
    held: HashMap<OpId, Fetching>,
}

struct Fetching {
    instance: EditorInstanceId,
    flow: Flow,
    /// The head once it arrived, or why none did.
    head: Option<Result<ResponseHead, Failure>>,
    asked: Option<Reply<Result<ResponseHead, Failure>>>,
    /// The `body` stream, once the app asked for it.
    body: Option<OpId>,
    /// Encoded body items not yet yielded.
    items: VecDeque<Vec<u8>>,
    /// The body is whole, or failed: once `items` drains, the stream completes.
    ended: bool,
}

impl Fetching {
    fn new(instance: EditorInstanceId) -> Self {
        Self {
            instance,
            flow: Flow::default(),
            head: None,
            asked: None,
            body: None,
            items: VecDeque::new(),
            ended: false,
        }
    }

    fn answer_head(&mut self, answers: &mut Vec<(OpId, Outcome)>) {
        if let Some(head) = &self.head
            && let Some(reply) = self.asked.take()
        {
            answers.push(reply.answer(head));
        }
    }

    /// The request failed, or the rest of its body did not arrive: the body ends with why.
    fn fail(&mut self, failure: Failure) {
        if self.ended {
            return;
        }
        let item: BodyItem = Err(failure.clone());
        self.items
            .push_back(wire::encode::<BodyEncoding, BodyItem>(&item));
        self.head.get_or_insert(Err(failure));
        self.ended = true;
    }

    /// Ends whatever the app still waits on, as canceled.
    fn cancel(self, answers: &mut Vec<(OpId, Outcome)>) {
        self.flow.abort();
        if let Some(reply) = self.asked {
            answers.push((reply.into_op(), Outcome::Err(OpError::Canceled)));
        }
        if let Some(body) = self.body {
            answers.push((body, Outcome::Err(OpError::Canceled)));
        }
    }
}

fn headers(headers: Vec<(String, String)>) -> Vec<Header> {
    headers
        .into_iter()
        .map(|(name, value)| Header { name, value })
        .collect()
}

impl Instances {
    pub fn network(&mut self, instance: EditorInstanceId, call: Call) {
        if !self.entries.contains_key(&instance) {
            let refused = call.refuse(OpError::Canceled);
            self.services.answers.push(refused);
            return;
        }
        match call {
            Call::FetchNew { resource, request } => {
                let op = resource.op();
                let mut fetching = Fetching::new(instance);
                if !allowed(&request.url, &self.network) {
                    fetching.fail(Failure::Denied);
                    self.fetches.held.insert(op, fetching);
                    return;
                }
                let post = self.mail.fetched.1.clone();
                let epoch = self.epoch;
                let request = http::Request {
                    method: request.method,
                    url: request.url,
                    headers: request
                        .headers
                        .into_iter()
                        .map(|header| (header.name, header.value))
                        .collect(),
                    body: request.body,
                };
                http::send(request, fetching.flow.clone(), move |event| {
                    post.send(Arrived {
                        epoch,
                        fetch: op,
                        event,
                    })
                });
                self.fetches.held.insert(op, fetching);
            }
            Call::FetchResponse { this, reply } => {
                let Some(fetching) = self.fetches.held.get_mut(&this.op()) else {
                    self.services
                        .answers
                        .push((reply.into_op(), Outcome::Err(OpError::Canceled)));
                    return;
                };
                if let Some(earlier) = fetching.asked.replace(reply) {
                    self.services
                        .answers
                        .push((earlier.into_op(), Outcome::Err(OpError::Canceled)));
                }
                fetching.answer_head(&mut self.services.answers);
            }
            Call::FetchBody { this, stream } => {
                let Some(fetching) = self.fetches.held.get_mut(&this.op()) else {
                    self.services
                        .answers
                        .push((stream.op(), Outcome::Err(OpError::Canceled)));
                    return;
                };
                if let Some(earlier) = fetching.body.replace(stream.op()) {
                    self.services
                        .answers
                        .push((earlier, Outcome::Err(OpError::Canceled)));
                }
            }
            Call::OpenUrl { reply, url } => {
                let opened = platform::open_link(&url).map_err(Failure::Failed);
                self.services.answers.push(reply.answer(&opened));
            }
        }
    }

    pub(super) fn arrived(&mut self, arrived: Arrived) {
        if arrived.epoch != self.epoch {
            return;
        }
        let Some(fetching) = self.fetches.held.get_mut(&arrived.fetch) else {
            return;
        };
        match arrived.event {
            Event::Head(head) => {
                fetching.head = Some(Ok(ResponseHead {
                    status: head.status,
                    headers: headers(head.headers),
                }));
            }
            Event::Chunk(chunk) => fetching
                .items
                .push_back(wire::encode::<BodyEncoding, BodyItem>(&Ok(chunk))),
            Event::Done => fetching.ended = true,
            Event::Failed(error) => fetching.fail(Failure::Failed(error)),
        }
        fetching.answer_head(&mut self.services.answers);
    }

    /// Yields what each fetch's body stream has credit for (`send` answers whether it had),
    /// and answers the streams whose body is whole.
    pub fn stream_fetches(
        &mut self,
        mut send: impl FnMut(OpId, &[u8]) -> bool,
    ) -> Vec<(OpId, Outcome)> {
        let mut finished = Vec::new();
        for fetching in self.fetches.held.values_mut() {
            let Some(body) = fetching.body else {
                continue;
            };
            while let Some(item) = fetching.items.front() {
                if !send(body, item) {
                    break;
                }
                fetching.items.pop_front();
                fetching.flow.release();
            }
            if fetching.ended && fetching.items.is_empty() {
                fetching.body = None;
                finished.push((body, Outcome::returned(&())));
            }
        }
        finished
    }

    /// The app dropped a fetch, or gave up on its response or body: the request stops.
    /// Answers whether `op` was one.
    pub(super) fn fetch_ended(&mut self, op: OpId) -> bool {
        let fetch = self.fetches.held.iter().find_map(|(fetch, fetching)| {
            let asked = fetching.asked.as_ref().map(Reply::op);
            (*fetch == op || asked == Some(op) || fetching.body == Some(op)).then_some(*fetch)
        });
        let Some(fetching) = fetch.and_then(|fetch| self.fetches.held.remove(&fetch)) else {
            return false;
        };
        fetching.cancel(&mut self.services.answers);
        true
    }

    pub(super) fn forget_fetches(&mut self, instance: Option<EditorInstanceId>) {
        let ended: Vec<OpId> = self
            .fetches
            .held
            .iter()
            .filter(|(_, fetching)| instance.is_none_or(|instance| fetching.instance == instance))
            .map(|(fetch, _)| *fetch)
            .collect();
        for fetch in ended {
            if let Some(fetching) = self.fetches.held.remove(&fetch) {
                fetching.cancel(&mut self.services.answers);
            }
        }
    }
}
