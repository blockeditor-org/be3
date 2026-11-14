use super::*;

/// The owner took a follower's edit and sealed it while the follower heard nothing. Caught up,
/// the follower holds the edit in the snapshot's state yet still has it pending, so it submits it
/// again; the owner, which took it already, tells it so rather than ignoring it, and the edit
/// shows once and settles.
#[test]
fn a_followers_edit_the_owner_took_and_sealed_while_it_was_deaf_lands_once() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("start"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["start"]);

    sim.deaf.insert(1);
    sim.operate(1, 1, add("mine"));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["start", "mine"]);
    assert!(
        !sim.devices[1].status.clean,
        "the follower has not heard that its edit was taken"
    );

    sim.deaf.clear();
    sim.resynchronize(1);
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["start", "mine"]);
    assert_eq!(texts(&sim.content(1)), ["start", "mine"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["start", "mine"]);
    assert!(sim.devices[1].status.clean, "the follower's edit settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
