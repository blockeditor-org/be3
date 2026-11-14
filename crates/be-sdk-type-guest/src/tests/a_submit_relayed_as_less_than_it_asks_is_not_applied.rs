use super::*;

use be_block_commit::MergeResult;
use be_block_content::{ContentError, Merge};
use be_block_session::{OpId, Payload, SessionMessage};
use serde::{Deserialize, Serialize};

/// Each operation adds its number.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Tally {
    entries: Vec<u32>,
}

impl BlockContent for Tally {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7461_6c6c_7900_0000_0000_0000_0000_0001);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("tally"))
    }
}

impl LiveEdit for Tally {
    type Op = u32;

    fn apply(&mut self, operation: &u32) {
        self.entries.push(*operation);
    }
}

impl Merge for Tally {
    fn merge3(_: &Self, ours: &Self, _: &Self) -> MergeResult<Self> {
        MergeResult::Clean(ours.clone())
    }
}

fn tally_module() -> Module {
    Module::new(vec![Registration::of::<Tally>()])
}

fn entries(sim: &mut Sim<Tally>, device: usize) -> Vec<u32> {
    let ticket = sim.with_replica(device, |replica, host| replica.content(host));
    let outcome = sim.answered(device, ticket.op());
    let bytes = ticket
        .decode(outcome)
        .expect("content is answered")
        .expect("the replica is readable");
    Tally::decode(&bytes).unwrap().entries
}

fn submit(client: ClientId, number: u32) -> Vec<u8> {
    crate::wire::Wire::Session(SessionMessage::Submit {
        id: OpId {
            client,
            counter: 1_000,
        },
        base: 0,
        payload: Payload::Inline(Tally::encode_operation(&number)),
    })
    .encode()
}

/// The kind a relay names is its sender's label, so the server stamps what it checked and the
/// owner drops a `Submit` stamped as less than an edit: a viewer cannot edit by labelling one
/// as presence. An editor's is applied.
#[test]
fn a_submit_relayed_as_less_than_it_asks_is_not_applied() {
    let mut sim = Sim::<Tally>::new(3, tally_module);
    sim.devices[2].may_edit = false;
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let (editor, viewer) = (sim.devices[1].client, sim.devices[2].client);
    assert_eq!(sim.lease.owner(), Some(sim.devices[0].client));

    sim.relay_as(0, viewer, RelayKind::Peer, submit(viewer, 7));
    sim.settle();
    assert_eq!(
        entries(&mut sim, 0),
        Vec::<u32>::new(),
        "a viewer's Submit was applied"
    );

    sim.operate(2, 1, 8);
    sim.settle();
    assert_eq!(
        entries(&mut sim, 0),
        Vec::<u32>::new(),
        "the server relayed a viewer's edit"
    );

    sim.relay_as(0, editor, RelayKind::Edit, submit(editor, 9));
    sim.operate(1, 1, 10);
    sim.settle();
    for device in 0..2 {
        assert_eq!(entries(&mut sim, device), vec![9, 10], "device {device}");
    }
}
