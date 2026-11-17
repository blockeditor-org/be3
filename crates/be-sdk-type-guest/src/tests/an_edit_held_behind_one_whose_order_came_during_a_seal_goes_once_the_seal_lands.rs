use super::*;

/// An edit typed while the one before it waits for its place in the order goes once that place
/// comes, even when it comes while a seal waits on the server: once the seal lands, nothing is
/// left waiting, and the edit is submitted rather than kept until the next one is typed.
#[test]
fn an_edit_held_behind_one_whose_order_came_during_a_seal_goes_once_the_seal_lands() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "soup");
    sim.settle();

    // " and" is submitted and waits for its place; " bread" waits behind it.
    sim.stalled_submits = true;
    sim.type_at(0, 1, 4, " and");
    sim.settle();
    sim.type_at(0, 1, 8, " bread");
    sim.settle();
    // A seal of what is ordered so far waits on the server.
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.publishes, 0, "the seal waits on the server");

    // The order numbers " and" first, then the seal lands.
    sim.unstall();
    sim.settle();
    let status = &sim.devices[0].status;
    assert!(
        status.unjournaled.is_none(),
        "the edit held behind the numbered one was submitted: {:?}",
        status.unjournaled
    );
    assert!(status.journaled, "everything typed is in the order");
}
