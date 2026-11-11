use super::*;

/// A guard, not a reproduction: a pointer that moved there already passes on the code before
/// `PointerRested`, since the layout re-test sees an unchanged position. The test next to it,
/// which sends a rest, is the one that needs `PointerRested`.
#[test]
fn a_row_that_appears_under_a_still_pointer_is_hovered_without_being_pointed_at() {
    // The pointer is there before the row, whichever way it got there.
    let mut rows = menu();
    rows.harness
        .frame(vec![Input::PointerMoved(under_the_row())]);
    rows.shown.set(true);
    rows.harness.frame(Vec::new());
    assert_hovered_not_pointed(&rows, "a row that appears under a pointer that moved there");

    // Moving it is what points at the row.
    rows.harness
        .frame(vec![Input::PointerMoved(under_the_row() + vec2(1.0, 0.0))]);
    assert!(
        rows.pointed.get_untracked(),
        "a pointer that moves points at it"
    );
}
