use super::*;

#[test]
fn the_order_a_pointer_rests_and_a_row_appears_in_does_not_decide_what_is_pointed_at() {
    // The host reports a pointer that stays where it is while what is under it changes as
    // `PointerRested`: before the row exists, once it does, and in the frame it appears in.
    let mut first = menu();
    first
        .harness
        .frame(vec![Input::PointerRested(under_the_row())]);
    first.shown.set(true);
    first.harness.frame(Vec::new());
    assert_hovered_not_pointed(&first, "the rest came before the row");

    let mut after = menu();
    after.shown.set(true);
    after.harness.frame(Vec::new());
    after
        .harness
        .frame(vec![Input::PointerRested(under_the_row())]);
    assert_hovered_not_pointed(&after, "the rest came after the row");

    let mut together = menu();
    together.shown.set(true);
    together
        .harness
        .frame(vec![Input::PointerRested(under_the_row())]);
    assert_hovered_not_pointed(&together, "the rest came in the frame the row appeared in");
}
