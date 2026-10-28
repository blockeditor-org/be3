use super::*;
use crate::reactive::*;

#[test]
fn a_long_press_lifts_a_row_a_finger_then_moves() {
    let _root = RootScope::new();
    let state = ReorderLists {
        long_press: true,
        ..ReorderLists::new(vec![vec![1, 2, 3]])
    };
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    harness.touch(TouchPhase::Start, grab);
    harness.advance(crate::input::LONG_PRESS_DELAY);
    harness.frame(Vec::new());
    assert!(
        harness.document().dragging(),
        "holding the finger still lifts the row"
    );
    harness.touch(TouchPhase::Move, grab + vec2(0.0, 25.0));
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.1"),
        25.0,
        "and then it follows the finger along the list"
    );
    harness.touch(TouchPhase::End, grab + vec2(0.0, 25.0));
    assert_eq!(state.list(0), vec![2, 1, 3]);
}
