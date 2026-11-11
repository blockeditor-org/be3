use super::*;

/// A message naming a part longer than a replica will read is not waited for: the follower
/// asks to be caught up instead of reading it, and goes on.
#[test]
fn a_part_longer_than_any_edit_is_not_read() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let owner = sim.devices[0].client;
    let part = be_block_store::PartRef::Chunks(be_block_store::NodeRef {
        hash: Hash::of(b"never stored"),
        length: 1 << 40,
    });
    let op = be_block_session::SessionOp {
        id: be_block_session::OpId {
            client: owner,
            counter: 1,
        },
        sequence: 1,
        payload: be_block_session::Payload::Part(part.into()),
    };
    let wire = crate::wire::Wire::Session(be_block_session::SessionMessage::Accepted { op });
    sim.relay(1, owner, wire.encode());
    sim.settle();
    assert_eq!(sim.part_reads, 0, "nothing was read");
    sim.type_at(0, 1, 6, "typed\n");
    sim.settle();
    assert!(sim.typed(1, 1) == "start\ntyped\n");
}
