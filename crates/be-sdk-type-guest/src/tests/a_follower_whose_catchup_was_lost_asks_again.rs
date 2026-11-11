use super::*;

/// A catch-up the owner never heard (it was deaf) is asked for again after a wait, so the
/// follower does not sit ignoring the owner's messages.
#[test]
fn a_follower_whose_catchup_was_lost_asks_again() {
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
    sim.deaf.insert(0);
    sim.advance(30_000);
    sim.fail_reads = 0;
    sim.deaf.clear();
    sim.advance(120_000);
    let expected = format!("start\n{pasted}");
    for device in 0..2 {
        assert!(sim.typed(device, 1) == expected, "device {device}");
    }
    sim.type_at(1, 1, 0, "> ");
    sim.settle();
    assert!(sim.typed(0, 1).starts_with("> start"));
}
