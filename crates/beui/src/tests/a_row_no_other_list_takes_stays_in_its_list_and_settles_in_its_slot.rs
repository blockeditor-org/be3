use super::*;
use crate::reactive::*;

#[test]
fn a_row_no_other_list_takes_stays_in_its_list_and_settles_in_its_slot() {
    let _root = RootScope::new();
    let state = ReorderLists::new(vec![vec![1, 2, 3]]);
    let mut harness = Harness::new(reorder_lists(state));
    harness.settle();
    let grab = harness.center(harness.find("row.1"));
    let away = grab + vec2(150.0, 200.0);
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.settle();
    assert!(
        harness.document().find_test_id("preview.1").is_none(),
        "with nowhere else to go, the row does not turn into a floating preview"
    );
    let held = harness.rect(harness.find("row.1"));
    assert_eq!(held.left(), 0.0, "it stays in its list");
    assert_eq!(
        top_of(&harness, "row.2"),
        0.0,
        "and its neighbours part for it"
    );
    assert_eq!(top_of(&harness, "row.3"), REORDER_ROW);

    harness.release_at(away);
    harness.settle();
    assert_eq!(
        state.list(0),
        vec![2, 3, 1],
        "let go anywhere, it lands in the slot it was shown in"
    );
    assert_eq!(top_of(&harness, "row.1"), 2.0 * REORDER_ROW);
}
