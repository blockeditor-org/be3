use super::*;
use crate::reactive::*;

#[test]
fn a_row_carried_into_another_list_opens_a_gap_and_drops_into_it() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2], vec![3, 4]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    let into = harness.center(harness.find("row.4")) - vec2(0.0, 4.0);
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(30.0, 0.0))]);
    harness.frame(vec![Input::PointerMoved(into)]);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.2"),
        0.0,
        "the row it left closes the gap behind it"
    );
    assert_eq!(top_of(&harness, "row.3"), 0.0);
    assert_eq!(
        top_of(&harness, "row.4"),
        2.0 * REORDER_ROW,
        "the list it is over parts by the carried row's height before the row it would land ahead of"
    );

    harness.release_at(into);
    assert_eq!(state.list(0), vec![2]);
    assert_eq!(state.list(1), vec![3, 1, 4]);
    let moves = state.moves.get_untracked();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].0, 1, "the list it lands in reports the move");
    assert_eq!(moves[0].1.key, None, "as a row that came from elsewhere");
    assert_eq!(moves[0].1.after, Some(3));
    harness.frame(Vec::new());
    let landing = harness.rect(harness.find("row.1"));
    assert!(
        landing.left() > REORDER_COLUMN,
        "it lands in the other list"
    );
    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), REORDER_ROW);
    assert_eq!(top_of(&harness, "row.4"), 2.0 * REORDER_ROW);
}
