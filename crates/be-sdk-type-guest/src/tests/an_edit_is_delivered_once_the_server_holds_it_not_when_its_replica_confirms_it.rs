use super::*;

#[test]
fn an_edit_is_delivered_once_the_server_holds_it_not_when_its_replica_confirms_it() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);

    sim.operate(0, 1, add("alone"));
    sim.settle();
    let owner = &sim.editors[&(0, 1)];
    assert_eq!(owner.pending(), 0, "the owner's replica confirmed the edit");
    assert_eq!(
        owner.delivered(),
        0,
        "alone, the owner appends within APPEND_INTERVAL, and the server holds nothing yet"
    );
    sim.advance(crate::APPEND_INTERVAL + 1);
    assert_eq!(sim.editors[&(0, 1)].delivered(), 1);

    sim.open(1);
    sim.attach(1, 1, true);
    sim.deaf.insert(0);
    sim.operate(1, 1, add("on my way"));
    sim.settle();
    let follower = &sim.editors[&(1, 1)];
    assert_eq!(
        follower.pending(),
        0,
        "the follower's replica confirmed the edit"
    );
    assert_eq!(
        follower.delivered(),
        0,
        "no owner accepted the follower's edit, so the server lacks it"
    );

    sim.deaf.remove(&0);
    sim.resynchronize(1);
    sim.settle();
    assert_eq!(sim.editors[&(1, 1)].delivered(), 1);
    assert_eq!(texts(sim.mirror(1, 1)), ["alone", "on my way"]);
}
