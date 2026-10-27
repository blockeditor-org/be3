use super::*;
use crate::reactive::*;

#[test]
fn a_disabled_reorder_list_lifts_nothing() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    state.disabled.set(true);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 30.0))]);
    assert!(!harness.document().dragging());
    assert_eq!(top_of(&harness, "row.1"), 0.0);
    harness.release_at(grab + vec2(0.0, 30.0));
    assert_eq!(state.list(0), vec![1, 2, 3]);
}
