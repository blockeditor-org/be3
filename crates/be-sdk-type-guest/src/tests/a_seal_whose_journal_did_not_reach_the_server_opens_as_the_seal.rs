use super::*;

/// A device sealed while what it journaled stayed on it alone, and stopped before it heard the
/// seal land: the server's journal is still on the commit before, without those entries. Another
/// device opening the block loads the seal as it is, rather than replaying a journal that does
/// not reach it.
#[test]
fn a_seal_whose_journal_did_not_reach_the_server_opens_as_the_seal() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.type_at(1, 1, 0, "soup");
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.head_content()), "soup");

    sim.unheld_appends = true;
    sim.type_at(1, 1, 4, " and bread");
    sim.settle();
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(1, 1, 14, " and eggs");
    sim.settle();
    sim.advance(crate::APPEND_INTERVAL);
    sim.crash(1);
    sim.unstall();
    sim.unheld_appends = false;
    assert_eq!(prose(&sim.head_content()), "soup and bread");

    sim.open(0);
    assert_eq!(prose(&sim.content(0)), "soup and bread");
}
