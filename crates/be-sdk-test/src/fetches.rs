use std::collections::{BTreeMap, VecDeque};

use be_sdk_api::{
    Failure,
    embed::network::Call,
    network::{Request, ResponseHead},
    ops::{OpError, OpId, Outcome, Reply, wire},
};
use be_sdk_app_host::wire::Wire;

type BodyItem = Result<Vec<u8>, Failure>;
type BodyEncoding = wire::Fallible<wire::Bytes, wire::Plain>;

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
    /// The body's stream, by its op, once the editor asked for it.
    body: Option<OpId>,
    /// The body's items, each encoded as the stream yields it.
    items: VecDeque<Vec<u8>>,
    ended: bool,
}

/// The editor's fetches and the links it opened, answered through the editor's wire as
/// be-host's runtime answers them (`Instances::stream_fetches`): a body goes out in the chunks
/// the test gave, each once the editor has credit for it.
#[derive(Default)]
pub(crate) struct Fetches {
    held: BTreeMap<OpId, Held>,
    sent: Vec<Fetch>,
    pub(crate) opened: Vec<String>,
}

impl Fetches {
    pub(crate) fn has_sent(&self) -> bool {
        !self.sent.is_empty()
    }

    pub(crate) fn take_sent(&mut self) -> Vec<Fetch> {
        std::mem::take(&mut self.sent)
    }

    pub(crate) fn call(&mut self, call: Call, wire: &mut Wire) {
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
                None => wire.answer(reply.into_op(), Outcome::Err(OpError::Canceled)),
            },
            Call::FetchBody { this, stream } => match self.held.get_mut(&this.op()) {
                Some(held) => held.body = Some(stream.op()),
                None => wire.answer(stream.op(), Outcome::Err(OpError::Canceled)),
            },
            Call::OpenUrl { reply, url } => {
                let (op, outcome) = reply.answer(&Ok(()));
                wire.answer(op, outcome);
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
        held.items.extend(
            chunks
                .into_iter()
                .map(|chunk| wire::encode::<BodyEncoding, BodyItem>(&Ok(chunk))),
        );
        held.ended = true;
    }

    pub(crate) fn fail(&mut self, fetch: &Fetch, failure: Failure) {
        let held = self
            .held
            .get_mut(&fetch.op())
            .expect("the editor still holds the fetch the test fails");
        held.head.get_or_insert(Err(failure.clone()));
        held.items
            .push_back(wire::encode::<BodyEncoding, BodyItem>(&Err(failure)));
        held.ended = true;
    }

    /// Sends each response that was asked for, and as much of each body as its credit allows.
    pub(crate) fn answer(&mut self, wire: &mut Wire) {
        for held in self.held.values_mut() {
            if let Some(head) = &held.head
                && let Some(reply) = held.asked.take()
            {
                let (op, outcome) = reply.answer(head);
                wire.answer(op, outcome);
            }
            let Some(body) = held.body else {
                continue;
            };
            while let Some(item) = held.items.front() {
                if !wire.stream_item(body, item) {
                    break;
                }
                held.items.pop_front();
            }
            if held.ended && held.items.is_empty() {
                held.body = None;
                wire.answer(body, Outcome::returned(&()));
            }
        }
    }

    /// The editor dropped a fetch, which its wire has answered.
    pub(crate) fn ended(&mut self, op: OpId, wire: &mut Wire) {
        self.stop(op, wire);
    }

    /// Whether `op` is a fetch's response or body the editor gave up on, which its wire has
    /// answered: that stops the whole fetch, as the host stops it.
    pub(crate) fn withdrawn(&mut self, op: OpId, wire: &mut Wire) -> bool {
        let fetch = self.held.iter().find_map(|(fetch, held)| {
            let asked = held.asked.as_ref().is_some_and(|reply| reply.op() == op);
            let body = held.body == Some(op);
            (asked || body).then_some(*fetch)
        });
        let Some(fetch) = fetch else {
            return false;
        };
        self.stop(fetch, wire);
        true
    }

    /// Forgets the fetch whose resource is `fetch`, ending what the editor still waits on.
    fn stop(&mut self, fetch: OpId, wire: &mut Wire) {
        if let Some(held) = self.held.remove(&fetch) {
            if let Some(reply) = held.asked {
                wire.answer(reply.into_op(), Outcome::Err(OpError::Canceled));
            }
            if let Some(body) = held.body {
                wire.answer(body, Outcome::Err(OpError::Canceled));
            }
        }
        self.sent.retain(|sent| sent.op != fetch);
    }
}
