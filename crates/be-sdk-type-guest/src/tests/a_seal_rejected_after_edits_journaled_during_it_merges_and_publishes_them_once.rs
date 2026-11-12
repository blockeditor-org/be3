use super::*;

/// A seal the server rejects, because another device moved the head while it was on its way,
/// merges with that head: what was typed and journaled while it waited is published with the
/// merge, once, and nothing is left owing.
#[test]
fn a_seal_rejected_after_edits_journaled_during_it_merges_and_publishes_them_once() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "soup\n");
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.stalled = true;
    sim.type_at(0, 1, 5, "bread\n");
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(0, 1, 11, "eggs\n");
    sim.settle();
    sim.advance(crate::APPEND_INTERVAL);
    assert!(sim.devices[0].status.journaled);
    sim.quiet = true;
    sim.publish_elsewhere(&prose_of("soup\nmilk\n"));
    sim.quiet = false;

    sim.unstall();
    for _ in 0..4 {
        sim.advance(crate::QUIET_INTERVAL);
    }
    let saved = prose(&sim.head_content());
    for line in ["soup\n", "bread\n", "eggs\n", "milk\n"] {
        assert_eq!(saved.matches(line).count(), 1, "{line:?} once in {saved:?}");
    }
    assert!(sim.devices[0].status.clean, "nothing is left owing");
}
