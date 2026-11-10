use super::*;

#[test]
fn a_followers_edits_stay_lacking_through_a_takeover_until_its_seal() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.follow_lacking(1, 1);
    sim.deaf.insert(0);
    sim.operate(1, 1, add("written as the owner left"));
    sim.settle();
    assert_eq!(sim.lacking(1, 1), 1);

    sim.crash(0);
    sim.claim(1);
    sim.settle();
    assert_eq!(
        sim.lacking(1, 1),
        1,
        "the new owner sequences its unaccepted edit, which the server lacks until it is sealed"
    );
    sim.advance(crate::QUIET_INTERVAL + 1);
    assert_eq!(sim.lacking(1, 1), 0);
    assert_eq!(texts(&sim.head_content()), ["written as the owner left"]);
}
