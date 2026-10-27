use super::*;
use crate::reactive::*;

#[test]
fn escape_glides_a_lifted_row_back_and_moves_nothing() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 45.0))]);
    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), 45.0);
    assert_eq!(
        top_of(&harness, "row.3"),
        REORDER_ROW,
        "two rows passed, both moved up"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    assert!(!harness.document().dragging(), "Escape lets go of the row");
    harness.frame(Vec::new());
    let back = top_of(&harness, "row.1");
    assert!(
        back > 0.0 && back <= 45.0,
        "it glides back from where it was rather than jumping, at {back}"
    );
    harness.release_at(grab + vec2(0.0, 45.0));
    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), 0.0);
    assert_eq!(top_of(&harness, "row.3"), 2.0 * REORDER_ROW);
    assert_eq!(state.list(0), vec![1, 2, 3]);
    assert!(state.moves.get_untracked().is_empty(), "nothing was moved");
}
