use super::*;
use crate::reactive::*;

#[test]
fn the_gap_a_row_leaves_closes_over_frames() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3], vec![4]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.2"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(pos2(
        REORDER_COLUMN + 10.0,
        grab.y,
    ))]);
    assert_eq!(harness.document().find_test_id("row.2"), None);
    assert_eq!(
        top_of(&harness, "row.3"),
        2.0 * REORDER_ROW,
        "the row leaves its slot as a gap, so the list keeps its length that frame"
    );
    let mut last = 2.0 * REORDER_ROW;
    for _ in 0..3 {
        harness.frame(Vec::new());
        let now = top_of(&harness, "row.3");
        assert!(
            now < last && now > REORDER_ROW,
            "the gap closes a step each frame: {last} then {now}"
        );
        last = now;
    }
    harness.settle();
    assert_eq!(top_of(&harness, "row.3"), REORDER_ROW);
}
