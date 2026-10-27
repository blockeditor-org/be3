use super::*;
use crate::reactive::*;

#[test]
fn dragging_a_row_past_the_middle_of_the_next_reorders_the_list() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3, 4]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    assert_eq!(top_of(&harness, "row.1"), 0.0);

    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 8.0))]);
    harness.frame(Vec::new());
    assert!(
        harness.document().dragging(),
        "past the threshold the row lifts"
    );
    assert_eq!(
        top_of(&harness, "row.1"),
        8.0,
        "the lifted row follows the pointer along the list"
    );
    assert_eq!(
        top_of(&harness, "row.2"),
        REORDER_ROW,
        "and its neighbour stays until the row's middle passes the neighbour's"
    );

    harness.frame(vec![Input::PointerMoved(grab + vec2(30.0, 25.0))]);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.1"),
        25.0,
        "it moves only along the list, never across it"
    );
    let rising = top_of(&harness, "row.2");
    assert!(
        rising > 0.0 && rising < REORDER_ROW,
        "the neighbour it passed glides up into the gap, at {rising}"
    );
    assert_eq!(
        state.list(0),
        vec![1, 2, 3, 4],
        "nothing is committed while it is carried"
    );

    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 26.0))]);
    harness.release_at(grab + vec2(0.0, 26.0));
    assert_eq!(
        state.list(0),
        vec![2, 1, 3, 4],
        "the drop commits the new order"
    );
    let moves = state.moves.get_untracked();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].1.key, Some(1));
    assert_eq!(
        moves[0].1.after,
        Some(2),
        "reported as following the row it passed"
    );
    assert_eq!(moves[0].1.index, 1);
    harness.frame(Vec::new());
    let settling = top_of(&harness, "row.1");
    assert!(
        settling > REORDER_ROW && settling <= 26.0,
        "after the drop it settles from where it was let go, at {settling}"
    );
    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), REORDER_ROW);
    assert_eq!(top_of(&harness, "row.2"), 0.0);
}
