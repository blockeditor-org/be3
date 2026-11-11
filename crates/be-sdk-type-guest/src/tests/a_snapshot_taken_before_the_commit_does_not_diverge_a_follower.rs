use super::*;

/// While a part cannot be read and the owner's commit of the paste does not land, the
/// follower asks again, ever less often, and never diverges: once the read works it catches
/// up and edits.
#[test]
fn a_snapshot_taken_before_the_commit_does_not_diverge_a_follower() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.stalled = true;
    sim.fail_reads = usize::MAX;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.settle();
    for _ in 0..6 {
        sim.advance(30_000);
    }
    assert!(sim.typed(1, 1) == "start\n", "the follower waits");
    sim.fail_reads = 0;
    sim.unstall();
    sim.advance(60_000);
    let expected = format!("start\n{pasted}");
    for device in 0..2 {
        assert!(sim.typed(device, 1) == expected, "device {device}");
        assert!(prose(&sim.content(device)) == expected, "device {device}");
    }
    sim.type_at(1, 1, 0, "> ");
    sim.settle();
    assert!(
        sim.typed(0, 1).starts_with("> start"),
        "the follower still edits"
    );
}
