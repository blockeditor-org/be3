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
    let away = pos2(harness.center(harness.find("list.1")).x, grab.y + 30.0);
    harness.down(grab, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.2"),
        0.0,
        "the row folds out of its own list"
    );
    let preview = harness.rect(harness.find("preview.1"));
    let other = harness.rect(harness.find("list.1"));
    assert_eq!(
        preview.min,
        pos2(other.left(), away.y - 5.0),
        "the list it is over draws its preview in the gap it opens: lined up with its rows, \
         and under the pointer along it where the row was held"
    );
}
