use super::*;

/// A follower that reopens on the same connection numbers its next edit past the ones it made
/// before, so the owner, which refuses an id it took already, takes it.
#[test]
fn a_followers_edit_after_it_reopens_on_the_same_connection_lands() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("start"));
    sim.settle();
    sim.operate(1, 1, add("before"));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["start", "before"]);

    sim.reopen(1);
    sim.operate(1, 1, add("after"));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["start", "before", "after"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["start", "before", "after"]);
    assert!(sim.devices[1].status.clean, "the follower's edit settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
