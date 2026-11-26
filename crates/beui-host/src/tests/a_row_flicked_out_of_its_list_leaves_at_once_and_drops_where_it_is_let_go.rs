use super::*;
use crate::reactive::*;

#[test]
fn a_row_flicked_out_of_its_list_leaves_at_once_and_drops_where_it_is_let_go() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2], vec![3, 4]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    let over = harness.center(harness.find("list.1")).x;
    let between = pos2(over, grab.y);
    let into = harness.center(harness.find("row.4")) - vec2(0.0, 4.0);
    harness.down(grab, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(between)]);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.2"),
        0.0,
        "a row whose first move past the threshold is already over another list folds out of \
         its own, and the gap behind it closes"
    );

    harness.up(into, PointerButton::Primary);
    assert_eq!(state.list(0), vec![2]);
    assert_eq!(
        state.list(1),
        vec![3, 1, 4],
        "let go over the other list in the same frame as the last move, it drops where the button is let go"
    );
}
