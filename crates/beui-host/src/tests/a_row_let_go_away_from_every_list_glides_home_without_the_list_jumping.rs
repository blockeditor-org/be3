use super::*;
use crate::reactive::*;

#[test]
fn a_row_let_go_away_from_every_list_glides_home_without_the_list_jumping() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3], vec![4]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.2"));
    let away = pos2(REORDER_COLUMN + 10.0, grab.y + 30.0);
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.3"),
        REORDER_ROW,
        "pulled out of its list towards another that takes it, the row leaves no gap behind"
    );
    harness.release_at(away);
    assert!(!harness.document().dragging());
    let mut last = REORDER_ROW;
    for _ in 0..4 {
        harness.frame(Vec::new());
        let below = top_of(&harness, "row.3");
        assert!(
            below >= last && below < 2.0 * REORDER_ROW,
            "let go where nothing takes it, its slot opens again over frames: {last} then {below}"
        );
        last = below;
    }
    assert!(last > REORDER_ROW, "the slot is opening, at {last}");
    harness.settle();
    assert_eq!(top_of(&harness, "row.2"), REORDER_ROW);
    assert_eq!(harness.rect(harness.find("row.2")).left(), 0.0);
    assert_eq!(top_of(&harness, "row.3"), 2.0 * REORDER_ROW);
    assert_eq!(state.list(0), vec![1, 2, 3]);
    assert_eq!(state.list(1), vec![4]);
}
