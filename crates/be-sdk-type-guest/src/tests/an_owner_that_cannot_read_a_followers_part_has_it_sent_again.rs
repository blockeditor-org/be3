use super::*;

/// An owner that cannot read the part of a follower's edit for minutes gives up and tells
/// the follower to submit again, which lands once the reads work.
#[test]
fn an_owner_that_cannot_read_a_followers_part_has_it_sent_again() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.fail_reads = usize::MAX;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(1, 1, 6, &pasted);
    sim.settle();
    for _ in 0..12 {
        sim.advance(30_000);
    }
    assert!(sim.typed(0, 1) == "start\n", "the owner has not got it");
    sim.fail_reads = 0;
    sim.advance(60_000);
    let expected = format!("start\n{pasted}");
    for device in 0..2 {
        assert!(sim.typed(device, 1) == expected, "device {device}");
        assert!(prose(&sim.content(device)) == expected, "device {device}");
    }
    sim.type_at(0, 1, 0, "> ");
    sim.settle();
    assert!(sim.typed(1, 1).starts_with("> start"));
}
