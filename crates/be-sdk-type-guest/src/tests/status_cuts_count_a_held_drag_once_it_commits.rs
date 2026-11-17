use super::*;

/// What a replica says it took and the order holds counts the local edits a host keeps to give
/// again. While a drag is held its messages count as none, whatever is typed meanwhile; its
/// operations count once it commits, after what came before the commit.
#[test]
fn status_cuts_count_a_held_drag_once_it_commits() {
    let (mut sim, a, b) = super::tentative::dragging(1);
    let taken = sim.devices[0].status.taken;
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "one"));
    sim.operate(0, 1, Checklist::set_text(a, "two"));
    sim.settle();
    assert_eq!(sim.devices[0].status.taken, taken);
    sim.operate(0, 2, Checklist::set_text(b, "typed"));
    sim.settle();
    let status = sim.devices[0].status.clone();
    assert_eq!(status.taken, taken + 1, "the typed edit counts, the held ones do not");
    assert_eq!(status.journaled_taken, taken + 1);
    assert!(status.journaled);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    let status = sim.devices[0].status.clone();
    assert_eq!(status.taken, taken + 3, "the drag's two operations count on its commit");
    assert_eq!(status.journaled_taken, taken + 3);
    assert_eq!(texts(&sim.content(0)), ["two", "typed"]);
}
