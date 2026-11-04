use super::*;
use crate::reactive::*;

#[test]
fn a_finger_dragged_across_a_list_lifts_its_row() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    harness.touch(TouchPhase::Start, grab);
    harness.touch(TouchPhase::Move, grab + vec2(20.0, 0.0));
    harness.frame(Vec::new());
    assert!(
        harness.document().dragging(),
        "a finger across the list lifts the row"
    );
    harness.touch(TouchPhase::Move, grab + vec2(20.0, 25.0));
    harness.touch(TouchPhase::End, grab + vec2(20.0, 25.0));
    assert_eq!(state.list(0), vec![2, 1, 3]);
}
