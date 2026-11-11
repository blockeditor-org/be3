use super::*;

#[test]
fn two_quick_edits_folded_into_one_append_are_both_held_by_it() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.follow_lacking(0, 1);

    sim.operate(0, 1, add("first"));
    sim.operate(0, 1, add("second"));
    sim.settle();
    assert_eq!(sim.lacking(0, 1), 2);
    sim.advance(crate::APPEND_INTERVAL + 1);
    assert_eq!(
        sim.lacking(0, 1),
        0,
        "alone, the owner folds the two into one journal entry, which holds both"
    );
}
