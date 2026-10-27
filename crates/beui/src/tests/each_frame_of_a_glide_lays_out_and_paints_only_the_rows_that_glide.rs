use super::*;
use crate::reactive::*;

const ROWS: u64 = 20;

#[test]
fn each_frame_of_a_glide_lays_out_and_paints_only_the_rows_that_glide() {
    let _root = RootScope::new();
    let order = create_signal((1..=ROWS).collect::<Vec<_>>());
    let arm = create_signal(0_u64);
    let mut harness = Harness::new(gliding_rows(order, arm));
    harness.settle();

    let mut swapped = (1..=ROWS).collect::<Vec<_>>();
    swapped.swap(0, 1);
    order.set(swapped);
    arm.update(|arm| *arm += 1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let moving = top_of(&harness, "row.1");
    assert!(
        moving > 0.0 && moving < GLIDE_ROW,
        "row 1 is mid-glide at {moving}"
    );
    let work = harness.document().performance().latest.work;
    assert!(
        work.placed <= 4,
        "a frame of the glide lays out the two gliding rows and nothing else: {} nodes were \
         laid out",
        work.placed
    );
    assert!(
        work.painted_nodes <= 6,
        "and repaints only them: {} nodes were painted",
        work.painted_nodes
    );
}
