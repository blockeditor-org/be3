use super::tentative::told_ended;
use super::*;

/// A peer removes the item a drag here holds, and the session restarts before the drag ends:
/// the held edit does not carry onto the content loaded afresh, so the drag ends, its editor
/// is told, and its commit writes nothing.
#[test]
fn a_restart_that_removes_the_dragged_item_ends_it() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let (mut sim, a, b) = super::tentative::dragging(2);
    for round in 0..20 {
        let (scratch, created) = Checklist::add(&format!("scratch {round}"));
        sim.operate(0, 2, created);
        sim.operate(0, 2, Checklist::remove(scratch));
    }
    sim.settle();
    sim.gesture(1, 1, Gesture::BeginTentative);
    sim.operate(1, 1, Checklist::set_text(a, "held"));
    sim.operate(1, 1, Checklist::set_text(a, "held further"));
    sim.settle();
    sim.take_received(1, 1);
    sim.operate(0, 2, Checklist::remove(a));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    let restarted = sim.head;
    assert!(
        sim.journal.as_ref().is_some_and(|journal| {
            journal.base.commit == restarted
                && crate::wire::BaseState::decode(&journal.base.state)
                    .is_some_and(|state| state.content.len() < 8)
        }),
        "the holder restarted the session"
    );
    assert!(told_ended(&mut sim, 1, 1));

    sim.gesture(1, 1, Gesture::Commit);
    sim.operate(1, 2, Checklist::set_text(b, "after"));
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["after"]);
    }
}
