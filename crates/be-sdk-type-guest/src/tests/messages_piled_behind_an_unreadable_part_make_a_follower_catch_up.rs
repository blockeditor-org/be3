use super::*;

/// More messages than a replica holds behind a read are dropped, and the follower catches up
/// instead of holding them all.
#[test]
fn messages_piled_behind_an_unreadable_part_make_a_follower_catch_up() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let owner = sim.devices[0].client;
    let part = be_block_store::PartRef::Chunks(be_block_store::NodeRef {
        hash: Hash::of(b"never stored"),
        length: 2 << 20,
    });
    let accepted = |counter: u64, payload| {
        crate::wire::Wire::Session(be_block_session::SessionMessage::Accepted {
            op: be_block_session::SessionOp {
                id: be_block_session::OpId {
                    client: owner,
                    counter,
                },
                sequence: counter,
                payload,
            },
        })
        .encode()
    };
    sim.relay(
        1,
        owner,
        accepted(1, be_block_session::Payload::Part(part.into())),
    );
    for counter in 2..10_100 {
        sim.relay(
            1,
            owner,
            accepted(counter, be_block_session::Payload::Inline(vec![0])),
        );
    }
    sim.settle();
    sim.type_at(0, 1, 6, "typed\n");
    sim.settle();
    sim.advance(60_000);
    assert!(sim.typed(1, 1) == "start\ntyped\n");
    assert!(
        sim.typed(0, 1) == "start\ntyped\n",
        "the owner lost nothing"
    );
}
