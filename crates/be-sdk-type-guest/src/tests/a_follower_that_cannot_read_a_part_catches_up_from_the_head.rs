use super::*;

/// A part that is never readable does not leave the follower diverged: after the reads give
/// up it asks the owner to catch it up, and the owner's commit of the paste is its head.
#[test]
fn a_follower_that_cannot_read_a_part_catches_up_from_the_head() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.fail_reads = usize::MAX;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.settle();
    sim.advance(0);
    for _ in 0..4 {
        sim.advance(8_000);
    }
    let expected = format!("start\n{pasted}");
    assert!(prose(&sim.head_content()) == expected);
    for device in 0..2 {
        assert!(prose(&sim.content(device)) == expected, "device {device}");
        assert!(sim.typed(device, 1) == expected, "device {device}");
    }
    sim.fail_reads = 0;
    sim.type_at(1, 1, 0, "> ");
    sim.settle();
    assert!(
        sim.typed(0, 1).starts_with("> start"),
        "the follower still edits"
    );
}
