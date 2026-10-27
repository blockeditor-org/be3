use super::*;
use crate::reactive::*;

#[test]
fn a_glide_moved_again_mid_flight_carries_on_from_where_it_is() {
    let _root = RootScope::new();
    let order = create_signal(vec![1_u64, 2, 3]);
    let arm = create_signal(0_u64);
    let mut harness = Harness::new(gliding_rows(order, arm));
    harness.settle();

    order.set(vec![2, 3, 1]);
    arm.update(|arm| *arm += 1);
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    let before = top_of(&harness, "row.1");
    assert!(before > 0.0 && before < 2.0 * GLIDE_ROW);

    order.set(vec![1, 2, 3]);
    arm.update(|arm| *arm += 1);
    harness.frame(Vec::new());
    let after = top_of(&harness, "row.1");
    assert!(
        (after - before).abs() < GLIDE_ROW / 2.0,
        "sent back mid-flight, the row turns from where it is ({before}) rather than jumping, and is at {after}"
    );
    harness.frames_to_settle();
    assert_eq!(top_of(&harness, "row.1"), 0.0);
}
