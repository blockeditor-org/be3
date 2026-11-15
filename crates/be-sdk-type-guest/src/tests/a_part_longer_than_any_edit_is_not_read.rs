use super::*;

/// An entry naming a part longer than any edit is passed over by every replica alike, unread,
/// and the order goes on.
#[test]
fn a_part_longer_than_any_edit_is_not_read() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let part = be_block_store::PartRef::Chunks(be_block_store::NodeRef {
        hash: Hash::of(b"never stored"),
        length: 1 << 40,
    });
    sim.submit_as(
        0,
        Submitted {
            through: 0,
            entry: Entry::Operation(be_block_session::Payload::Part(part)),
        }
        .encode(),
    );
    assert_eq!(sim.part_reads, 0, "nothing was read");
    sim.type_at(0, 1, 6, "typed\n");
    sim.settle();
    assert_eq!(sim.typed(1, 1), "start\ntyped\n");
}
