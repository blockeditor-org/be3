use be_block_session::{OpId, Payload, SessionMessage, SessionOp};

use super::*;

/// What only the owner sends is ignored from anyone else, and a follower ignores what only
/// the owner acts on, so a participant that is not the owner cannot make a follower read a
/// part or move it.
#[test]
fn a_viewer_cannot_make_a_follower_read_a_part() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, device != 2);
    }
    let viewer = sim.devices[2].client;
    let part = be_block_store::PartRef::Chunks(be_block_store::NodeRef {
        hash: Hash::of(b"never stored"),
        length: 2 << 20,
    });
    let op = SessionOp {
        id: OpId {
            client: viewer,
            counter: 1,
        },
        sequence: 1,
        payload: Payload::Part(part.into()),
    };
    let head = sim.head.expect("published");
    let forged = [
        SessionMessage::Accepted { op: op.clone() },
        SessionMessage::Snapshot {
            head: Some(head),
            sealed: 0,
            sequence: 1,
            ops: vec![op.clone()],
            state: Vec::new(),
        },
        SessionMessage::Sealed {
            head,
            sequence: 1,
            reload: Some(Vec::new()),
            restart: true,
        },
        SessionMessage::Replaced { head },
        SessionMessage::Submit {
            id: op.id,
            base: 0,
            payload: op.payload.clone(),
        },
        SessionMessage::Catchup {
            since: 0,
            base: None,
            sealed: 0,
        },
        SessionMessage::Kept {
            commit: head,
            base: None,
        },
        SessionMessage::Resend,
    ];
    for message in forged {
        let name = format!("{message:?}");
        sim.relay(1, viewer, crate::wire::Wire::Session(message).encode());
        sim.settle();
        assert_eq!(sim.part_reads, 0, "{name} made the follower read");
    }
    sim.type_at(0, 1, 6, "typed\n");
    sim.settle();
    assert!(sim.typed(1, 1) == "start\ntyped\n");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
