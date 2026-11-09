use std::collections::{BTreeMap, VecDeque};

use be_sdk_api::{
    Failure,
    embed::network::Call,
    network::{Request, ResponseHead},
    ops::{Connection, OpError, OpId, Outcome, Producer, Reply, wire},
};

type BodyItem = Result<Vec<u8>, Failure>;
type Body = Producer<BodyItem, wire::Fallible<wire::Bytes, wire::Plain>>;

/// A request the editor sent with `EditorHost::fetch`, whole as it asked, which the test
/// answers with `BeuiTest::respond` or `BeuiTest::fail_fetch`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fetch {
    pub request: Request,
    op: OpId,
}

impl Fetch {
    pub(crate) fn op(&self) -> OpId {
        self.op
    }
}

struct Held {
    head: Option<Result<ResponseHead, Failure>>,
    asked: Option<Reply<Result<ResponseHead, Failure>>>,
    body: Option<Body>,
    items: VecDeque<BodyItem>,
    ended: bool,
}

/// The editor's fetches and the links it opened, answered as the host would: a body goes
/// out in the chunks the test gave, each once the editor has credit for it.
#[derive(Default)]
pub(crate) struct Fetches {
    held: BTreeMap<OpId, Held>,
    sent: Vec<Fetch>,
    pub(crate) opened: Vec<String>,
}

impl Fetches {
    pub(crate) fn holds(&self, op: OpId) -> bool {
        self.held.contains_key(&op)
    }

    pub(crate) fn has_sent(&self) -> bool {
        !self.sent.is_empty()
    }

    pub(crate) fn take_sent(&mut self) -> Vec<Fetch> {
        std::mem::take(&mut self.sent)
    }

    pub(crate) fn call(&mut self, call: Call, connection: &mut Connection) {
        match call {
            Call::FetchNew { resource, request } => {
                let op = resource.op();
                self.held.insert(
                    op,
                    Held {
                        head: None,
                        asked: None,
                        body: None,
                        items: VecDeque::new(),
                        ended: false,
                    },
                );
                self.sent.push(Fetch { request, op });
            }
            Call::FetchResponse { this, reply } => match self.held.get_mut(&this.op()) {
                Some(held) => held.asked = Some(reply),
                None => reply.fail(connection, OpError::Canceled),
            },
            Call::FetchBody { this, stream } => match self.held.get_mut(&this.op()) {
                Some(held) => held.body = Some(stream),
                None => stream.fail(connection, OpError::Canceled),
            },
            Call::OpenUrl { reply, url } => {
                reply.send(connection, &Ok(()));
                self.opened.push(url);
            }
        }
    }

    pub(crate) fn respond(
        &mut self,
        fetch: &Fetch,
        head: ResponseHead,
        chunks: impl IntoIterator<Item = Vec<u8>>,
    ) {
        let held = self
            .held
            .get_mut(&fetch.op())
            .expect("the editor still holds the fetch the test answers");
        held.head = Some(Ok(head));
        held.items.extend(chunks.into_iter().map(Ok));
        held.ended = true;
    }

    pub(crate) fn fail(&mut self, fetch: &Fetch, failure: Failure) {
        let held = self
            .held
            .get_mut(&fetch.op())
            .expect("the editor still holds the fetch the test fails");
        held.head.get_or_insert(Err(failure.clone()));
        held.items.push_back(Err(failure));
        held.ended = true;
    }

    /// Sends each response that was asked for, and as much of each body as its credit allows.
    pub(crate) fn answer(&mut self, connection: &mut Connection) {
        for held in self.held.values_mut() {
            if let Some(head) = &held.head
                && let Some(reply) = held.asked.take()
            {
                reply.send(connection, head);
            }
            let Some(body) = &held.body else {
                continue;
            };
            while let Some(item) = held.items.front() {
                if body.send(connection, item).is_err() {
                    break;
                }
                held.items.pop_front();
            }
            if held.ended
                && held.items.is_empty()
                && let Some(body) = held.body.take()
            {
                body.finish(connection);
            }
        }
    }

    /// The editor dropped a fetch, or gave up on its response or body.
    pub(crate) fn ended(&mut self, op: OpId, connection: &mut Connection) {
        self.stop(op, connection);
        connection.complete(op, Outcome::Err(OpError::Canceled));
    }

    /// Whether `op` is a fetch's response or body the editor gave up on, which stops the
    /// whole fetch, as the host stops it.
    pub(crate) fn withdrawn(&mut self, op: OpId, connection: &mut Connection) -> bool {
        let fetch = self.held.iter().find_map(|(fetch, held)| {
            let asked = held.asked.as_ref().is_some_and(|reply| reply.op() == op);
            let body = held.body.as_ref().is_some_and(|body| body.op() == op);
            (asked || body).then_some(*fetch)
        });
        let Some(fetch) = fetch else {
            return false;
        };
        self.stop(fetch, connection);
        true
    }

    /// Forgets the fetch whose resource is `fetch`, ending what the editor still waits on.
    fn stop(&mut self, fetch: OpId, connection: &mut Connection) {
        if let Some(held) = self.held.remove(&fetch) {
            if let Some(reply) = held.asked {
                reply.fail(connection, OpError::Canceled);
            }
            if let Some(body) = held.body {
                body.fail(connection, OpError::Canceled);
            }
        }
        self.sent.retain(|sent| sent.op != fetch);
    }
}
