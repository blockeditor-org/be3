use super::*;
use crate::reactive::*;

#[test]
fn a_row_let_go_away_from_every_list_glides_home() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.2"));
    let away = grab + vec2(150.0, 60.0);
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.3"),
        REORDER_ROW,
        "pulled out of its list, the row leaves no gap behind"
    );
    harness.release_at(away);
    assert!(!harness.document().dragging());
    harness.frame(Vec::new());
    let returning = harness.rect(harness.find("row.2")).left();
    assert!(
        returning > 0.0,
        "let go where nothing takes it, it glides back from there, at {returning}"
    );
    harness.settle();
    assert_eq!(top_of(&harness, "row.2"), REORDER_ROW);
    assert_eq!(harness.rect(harness.find("row.2")).left(), 0.0);
    assert_eq!(top_of(&harness, "row.3"), 2.0 * REORDER_ROW);
    assert_eq!(state.list(0), vec![1, 2, 3]);
}
