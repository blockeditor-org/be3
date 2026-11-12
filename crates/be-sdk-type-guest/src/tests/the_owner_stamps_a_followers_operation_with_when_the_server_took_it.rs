use super::*;

use be_block::{ContentError, Merge};
use be_block_commit::MergeResult;
use serde::{Deserialize, Serialize};

/// Each operation adds an entry, which keeps when the server took it if it came from a
/// follower (`LiveEdit::arrived`).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Arrivals {
    entries: Vec<Option<i64>>,
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
    type Op = Option<i64>;

    fn apply(&mut self, operation: &Option<i64>) {
        self.entries.push(*operation);
    }

    fn arrived(operation: &mut Option<i64>, received: i64) -> bool {
        *operation = Some(received);
        true
    }
}

impl Merge for Arrivals {
    fn merge3(_: &Self, ours: &Self, _: &Self) -> MergeResult<Self> {
        MergeResult::Clean(ours.clone())
    }
}

fn arrivals_module() -> Module {
    Module::new(vec![Registration::of::<Arrivals>()])
}

fn entries(sim: &mut Sim<Arrivals>, device: usize) -> Vec<Option<i64>> {
    let ticket = sim.with_replica(device, |replica, host| replica.content(host));
    let outcome = sim.answered(device, ticket.op());
    let bytes = ticket
        .decode(outcome)
        .expect("content is answered")
        .expect("the replica is readable");
    Arrivals::decode(&bytes).unwrap().entries
}

#[test]
fn the_owner_stamps_a_followers_operation_with_when_the_server_took_it() {
    let mut sim = Sim::<Arrivals>::new(2, arrivals_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);

    sim.operate(0, 1, None);
    sim.settle();
    sim.advance(500);
    let sent = sim.now as i64;
    sim.operate(1, 1, None);
    sim.settle();

    let expected = vec![None, Some(sent)];
    for device in [0, 1] {
        assert_eq!(
            entries(&mut sim, device),
            expected,
            "device {device}: the owner's own edit keeps no stamp, and the follower's has the \
             time the server took it"
        );
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
