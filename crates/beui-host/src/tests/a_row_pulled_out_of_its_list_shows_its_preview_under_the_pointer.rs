use super::*;
use crate::reactive::*;

#[test]
fn a_row_pulled_out_of_its_list_shows_its_preview_under_the_pointer() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2], vec![3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let row = harness.rect(harness.find("row.1"));
    let grab = row.min + vec2(30.0, 5.0);
    let away = pos2(REORDER_COLUMN + 10.0, grab.y + 30.0);
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.settle();
    assert_eq!(harness.document().find_test_id("row.1"), None);
    let preview = harness.rect(harness.find("preview.1"));
    assert_eq!(
        preview.min,
        away - vec2(30.0, 5.0),
        "the preview sits under the pointer where the row was held"
    );
}
