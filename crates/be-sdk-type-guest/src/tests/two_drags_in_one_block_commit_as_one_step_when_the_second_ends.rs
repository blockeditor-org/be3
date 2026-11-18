use super::tentative::dragging;
use super::*;

/// Two editors drag in one block at once: each previews its drag from the block, nothing is
/// sent while either holds, the first commit waits for the second, and both go out together as
/// one undo step.
#[test]
fn two_drags_in_one_block_commit_as_one_step_when_the_second_ends() {
    let (mut sim, a, b) = dragging(2);
    let (first, second) = (
        Checklist::set_text(a, "first"),
        Checklist::set_text(b, "second"),
    );
    let taken = sim.devices[0].status.taken;
    let ordered = sim.journal_operations().len();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, first.clone());
    sim.gesture(0, 2, Gesture::BeginTentative);
    sim.operate(0, 2, second.clone());
    sim.settle();
    for editor in [1, 2] {
        assert_eq!(texts(sim.mirror(0, editor)), ["first", "second"]);
    }
    assert_eq!(
        texts(&sim.content(0)),
        ["a", "b"],
        "held edits are never written"
    );
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    assert_eq!(
        sim.journal_operations().len(),
        ordered,
        "a committed drag waits for the drag still held"
    );
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);

    sim.gesture(0, 2, Gesture::Commit);
    sim.settle();
    // Both go out, absorbed into one edit.
    assert!(sim.journal_operations().len() > ordered);
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["first", "second"]);
    }
    assert_eq!(
        sim.devices[0].status.taken,
        taken + 2,
        "the two drags' edits count, and their begins and commits do not"
    );

    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert_eq!(
        texts(&sim.content(0)),
        ["a", "b"],
        "the two drags undo as one step"
    );
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);
}
