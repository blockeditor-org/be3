use super::*;
use crate::reactive::*;

#[test]
fn alt_and_an_arrow_move_the_focused_row_and_say_where_it_went() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    harness.click(harness.center(harness.find("row.1")));
    harness.key(Key::ArrowDown, Modifiers::ALT);
    assert_eq!(state.list(0), vec![2, 1, 3]);
    harness.key(Key::ArrowDown, Modifiers::ALT);
    assert_eq!(
        state.list(0),
        vec![2, 3, 1],
        "the row keeps its focus as it moves"
    );
    harness.key(Key::ArrowDown, Modifiers::ALT);
    assert_eq!(state.list(0), vec![2, 3, 1], "and stops at the end");
    harness.key(Key::ArrowUp, Modifiers::ALT);
    assert_eq!(state.list(0), vec![2, 1, 3]);
    harness.frame(Vec::new());
    let said = harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == accesskit::Role::Status)
        .and_then(|node| node.label().map(str::to_owned));
    assert_eq!(said.as_deref(), Some("Row 1 moved to position 2 of 3"));
    harness.frame(Vec::new());
    let moving = top_of(&harness, "row.1");
    assert!(moving < REORDER_ROW * 2.0, "the move glides, at {moving}");
    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), REORDER_ROW);

    state.disabled.set(true);
    harness.key(Key::ArrowUp, Modifiers::ALT);
    assert_eq!(
        state.list(0),
        vec![2, 1, 3],
        "a disabled list does not move"
    );
}
