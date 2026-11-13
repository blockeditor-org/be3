use super::*;
use crate::reactive::*;

#[test]
fn a_scroll_in_the_update_that_arms_a_glide_moves_no_row() {
    let _root = RootScope::new();
    let order = create_signal(vec![1_u64, 2, 3, 4]);
    let arm = create_signal(0_u64);
    let scrolled = create_signal(0.0_f32);
    let mut harness = Harness::new(scrolled_gliding_rows(order, arm, scrolled));
    harness.settle();
    assert_eq!(top_of(&harness, "row.4"), 3.0 * GLIDE_ROW);

    order.set(vec![2, 1, 3, 4]);
    arm.update(|arm| *arm += 1);
    scrolled.set(GLIDE_ROW);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.4"),
        2.0 * GLIDE_ROW,
        "a row the reorder left in its place is drawn where the scroll put it, not glided by it"
    );
    harness.frame(Vec::new());
    let moving = top_of(&harness, "row.1");
    assert!(
        moving < 0.0,
        "a row it moved glides from where it was, scrolled with the rest, though the scroll left \
         it where it was drawn: at {moving}"
    );

    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), 0.0);
    assert_eq!(top_of(&harness, "row.2"), -GLIDE_ROW);
}
