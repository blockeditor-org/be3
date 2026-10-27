use super::*;
use crate::reactive::*;

#[test]
fn a_glide_armed_by_a_reorder_paints_where_it_was_and_springs_to_its_new_place() {
    let _root = RootScope::new();
    let order = create_signal(vec![1_u64, 2, 3]);
    let arm = create_signal(0_u64);
    let mut harness = Harness::new(gliding_rows(order, arm));
    harness.settle();
    assert_eq!(top_of(&harness, "row.1"), 0.0);

    order.set(vec![2, 3, 1]);
    arm.update(|arm| *arm += 1);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "slot.1"),
        2.0 * GLIDE_ROW,
        "the glide itself rests in its new place at once"
    );
    assert_eq!(
        top_of(&harness, "row.1"),
        0.0,
        "but the first frame after the move still paints the row where it was"
    );
    assert_eq!(top_of(&harness, "row.2"), GLIDE_ROW);

    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let moving = top_of(&harness, "row.1");
    assert!(
        moving > 0.0 && moving < 2.0 * GLIDE_ROW,
        "a few frames on, the row is on its way, at {moving}"
    );
    let rising = top_of(&harness, "row.2");
    assert!(
        rising < GLIDE_ROW && rising > 0.0,
        "and the row it passed glides up towards the top, at {rising}"
    );

    let frames = harness.frames_to_settle();
    assert!(frames < 60, "the glide comes to rest within a second, after {frames} frames");
    assert_eq!(top_of(&harness, "row.1"), 2.0 * GLIDE_ROW);
    assert_eq!(top_of(&harness, "row.2"), 0.0);
}
