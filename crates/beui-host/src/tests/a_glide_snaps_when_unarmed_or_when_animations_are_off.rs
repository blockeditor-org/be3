use super::*;
use crate::reactive::*;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn a_glide_snaps_when_unarmed_or_when_animations_are_off() {
    let _root = RootScope::new();
    let order = create_signal(vec![1_u64, 2, 3]);
    let arm = create_signal(0_u64);
    let mut harness = Harness::new(gliding_rows(order, arm));
    harness.settle();

    order.set(vec![2, 3, 1]);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.1"),
        2.0 * GLIDE_ROW,
        "a move its owner did not arm the glide for lands at once"
    );
    assert_eq!(harness.frames_to_settle(), 0, "and starts nothing");

    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    order.set(vec![1, 2, 3]);
    arm.update(|arm| *arm += 1);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.1"),
        0.0,
        "with animations off, an armed move lands at once too"
    );
    assert_eq!(harness.frames_to_settle(), 0);
}
