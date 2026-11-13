//! A content type that keeps when the server took each note (`LiveEdit::arrived`), for the
//! arrival-time tests.

use super::*;

use be_block::{ContentError, Merge};
use be_block_commit::MergeResult;
use serde::{Deserialize, Serialize};

/// A note: the stamp the owner gives it, and bytes to make it as large as a test needs.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct Note {
    pub(super) stamp: Option<i64>,
    #[serde(with = "serde_bytes")]
    pub(super) padding: Vec<u8>,
}

impl Note {
    pub(super) fn sized(padding: usize) -> Self {
        Self {
            stamp: None,
            padding: vec![0; padding],
        }
    }
}

/// The stamp of each note, in order.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct Arrivals {
    pub(super) entries: Vec<Option<i64>>,
}

impl BlockContent for Arrivals {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6172_7269_7661_6c73_0000_0000_0000_0001);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("arrivals"))
    }
}

impl LiveEdit for Arrivals {
    type Op = Note;

    fn apply(&mut self, operation: &Note) {
        self.entries.push(operation.stamp);
    }

    fn claimed_arrival(operation: &Note) -> Option<i64> {
        operation.stamp
    }

    fn arrived(operation: &mut Note, received: i64) -> bool {
        operation.stamp = Some(received);
        true
    }
}

impl Merge for Arrivals {
    fn merge3(_: &Self, ours: &Self, _: &Self) -> MergeResult<Self> {
        MergeResult::Clean(ours.clone())
    }
}

pub(super) fn arrivals_module() -> Module {
    Module::new(vec![Registration::of::<Arrivals>()])
}

/// The stamps `device`'s replica holds.
pub(super) fn entries(sim: &mut Sim<Arrivals>, device: usize) -> Vec<Option<i64>> {
    let ticket = sim.with_replica(device, |replica, host| replica.content(host));
    let outcome = sim.answered(device, ticket.op());
    let bytes = ticket
        .decode(outcome)
        .expect("content is answered")
        .expect("the replica is readable");
    Arrivals::decode(&bytes).unwrap().entries
}

/// A `Submit` from `client` of `payload`, as a forger would send it.
pub(super) fn submit(client: ClientId, counter: u64, payload: be_block_session::Payload) -> Vec<u8> {
    crate::wire::Wire::Session(be_block_session::SessionMessage::Submit {
        id: be_block_session::OpId { client, counter },
        base: 0,
        payload,
    })
    .encode()
}
