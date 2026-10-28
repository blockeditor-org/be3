use super::*;
use crate::reactive::*;

#[test]
fn a_cancelled_pointer_puts_a_held_row_back_where_it_was() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    harness.touch(TouchPhase::Start, grab);
    harness.touch(TouchPhase::Move, grab + vec2(30.0, 0.0));
    harness.touch(TouchPhase::Move, grab + vec2(30.0, 50.0));
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.2"),
        0.0,
        "the held row has passed two rows"
    );
    harness.touch(TouchPhase::Cancel, grab + vec2(30.0, 50.0));
    harness.settle();
    assert!(!harness.document().dragging());
    assert_eq!(
        state.list(0),
        vec![1, 2, 3],
        "a cancelled drag moves nothing"
    );
    assert_eq!(top_of(&harness, "row.1"), 0.0);
}
