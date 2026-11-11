use super::*;

/// A message only the owner sends is ignored from anyone else, so a participant that is not
/// the owner cannot make a follower read a part.
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
    let op = be_block_session::SessionOp {
        id: be_block_session::OpId {
            client: viewer,
            counter: 1,
        },
        sequence: 1,
        payload: be_block_session::Payload::Part(part.into()),
    };
    let wire = crate::wire::Wire::Session(be_block_session::SessionMessage::Accepted { op });
    sim.relay(1, viewer, wire.encode());
    sim.settle();
    assert_eq!(sim.part_reads, 0, "nothing was read");
    sim.type_at(0, 1, 6, "typed\n");
    sim.settle();
    assert!(sim.typed(1, 1) == "start\ntyped\n");
}
