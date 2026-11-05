use super::*;

#[test]
fn a_journal_another_build_wrote_is_dropped_and_the_block_marked() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("written by the old build"));
    sim.advance(crate::APPEND_INTERVAL);
    assert!(sim.journal.as_ref().is_some_and(|journal| !journal.entries.is_empty()));
    sim.crash(0);

    sim.devices[1].tag = SessionTag([9; 32]);
    sim.open(1);
    sim.attach(1, 1, true);
    assert!(
        texts(&sim.content(1)).is_empty(),
        "a build cannot replay another build's journal"
    );
    assert!(sim.devices[1].undo.lost_edits, "the block says edits were lost");

    sim.operate(1, 1, add("written by the new build"));
    sim.advance(crate::APPEND_INTERVAL);
    assert_eq!(sim.journal_tag, SessionTag([9; 32]));
    let journal = sim.journal.clone().expect("the new build journals");
    assert!(
        journal.base.commit == sim.head && journal.entries.len() == 1,
        "the new build started the journal over: {journal:?}"
    );
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(texts(&sim.head_content()), ["written by the new build"]);
}
