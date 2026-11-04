use super::*;
use crate::reactive::*;

#[test]
fn a_list_refuses_another_lists_rows_it_does_not_accept_and_nothing_in_it_moves() {
    let _root = RootScope::new();
    let state = ReorderLists {
        refuses: Some(2),
        ..ReorderLists::new(vec![vec![1, 2], vec![3], vec![4, 5]])
    };
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    let over = harness.center(harness.find("row.5")) - vec2(0.0, 4.0);
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(30.0, 0.0))]);
    harness.frame(vec![Input::PointerMoved(over)]);
    harness.settle();
    assert_eq!(top_of(&harness, "row.4"), 0.0);
    assert_eq!(
        top_of(&harness, "row.5"),
        REORDER_ROW,
        "a list that does not accept the row opens no gap for it"
    );

    harness.release_at(over);
    harness.settle();
    assert_eq!(state.list(2), vec![4, 5], "and nothing in it moves");
    assert_eq!(
        state.list(0),
        vec![1, 2],
        "the row goes back where it came from"
    );
    assert!(state.moves.get_untracked().is_empty());
}
