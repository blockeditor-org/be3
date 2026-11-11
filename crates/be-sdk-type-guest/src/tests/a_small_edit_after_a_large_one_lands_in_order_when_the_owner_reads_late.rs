use super::*;

/// A follower's large paste and a small edit after it, while the owner's first read of the
/// paste fails: every replica ends with the same text, so numbering the small edit before
/// the paste is safe for positions.
#[test]
fn a_small_edit_after_a_large_one_lands_in_order_when_the_owner_reads_late() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.fail_reads = 1;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(1, 1, 6, &pasted);
    sim.type_at(1, 1, 0, "> ");
    sim.type_at(0, 1, 0, "# ");
    sim.settle();
    sim.advance(10_000);
    let expected = sim.typed(0, 1);
    assert_eq!(expected.len(), 6 + 2 + 2 + pasted.len());
    for device in 0..3 {
        assert!(sim.typed(device, 1) == expected, "device {device}");
        assert!(prose(&sim.content(device)) == expected, "device {device}");
    }
    assert!(expected.contains("start\n"));
}
