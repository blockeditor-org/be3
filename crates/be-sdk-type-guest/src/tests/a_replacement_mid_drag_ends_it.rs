use super::tentative::told_ended;
use super::*;

/// The content is replaced while a drag is held: the drag ends, its editor is told and cancels
/// it, drops what it makes in it after, and nothing of it is written.
#[test]
fn a_replacement_mid_drag_ends_it() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "held"));
    sim.settle();
    sim.take_received(0, 1);

    let mut replacement = ChecklistContent::default();
    let (_, added) = Checklist::add("replaced");
    replacement.apply(&added);
    assert!(sim.change(1, Change::Replace(replacement.encode())));
    sim.settle();
    assert!(told_ended(&mut sim, 0, 1));
    assert_eq!(texts(sim.mirror(0, 2)), ["replaced"]);

    assert!(
        !sim.operate_in(0, 1, Some(0), Checklist::set_text(a, "later")),
        "the editor heard, and drops what it makes in the gesture"
    );
    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["replaced"]);
    }
    assert_eq!(texts(sim.mirror(0, 2)), ["replaced"]);
}
