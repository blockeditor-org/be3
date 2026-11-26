use super::*;

#[test]
fn a_new_holder_recovers_edits_from_the_journal_of_one_that_crashed() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    for text in ["milk", "eggs", "bread"] {
        sim.operate(0, 1, add(text));
        sim.advance(100);
    }
    sim.settle();
    assert_eq!(sim.publishes, 0, "nothing is committed before a pause");
    assert!(sim.devices[0].status.journaled);
    sim.crash(0);

    sim.open(1);
    assert_eq!(
        texts(&sim.content(1)),
        ["milk", "eggs", "bread"],
        "a replica opening on the journal's base replays it"
    );
    assert!(sim.devices[1].status.holder);
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.publishes, 1);
    assert_eq!(texts(&sim.head_content()), ["milk", "eggs", "bread"]);
    assert!(
        sim.journal
            .as_ref()
            .is_some_and(|journal| journal.entries.is_empty() && journal.base.commit == sim.head),
        "the commit starts the journal over from itself"
    );
}
