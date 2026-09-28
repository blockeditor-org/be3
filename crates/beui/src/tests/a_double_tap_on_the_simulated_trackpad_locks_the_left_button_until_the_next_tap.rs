use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn a_double_tap_on_the_simulated_trackpad_locks_the_left_button_until_the_next_tap() {
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider value=0.0 />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();

    let track = harness.rect(slider);
    harness.point_at(pos2(track.left() + 1.0, track.center().y));
    harness.quick_tap_trackpad();
    harness.quick_tap_trackpad();
    harness.wait_out_double_tap();
    assert!(harness.simulated_left_held(), "the double tap locks the button");

    let at = harness.simulated_trackpad();
    let half = (track.center() - harness.simulated_cursor()) * 0.5;
    for finger in [1, 2] {
        harness.finger(finger, TouchPhase::Start, at);
        harness.finger(finger, TouchPhase::Move, at + half);
        harness.finger(finger, TouchPhase::End, at + half);
        harness.frame(Vec::new());
    }
    assert!(harness.simulated_left_held(), "lifting a moving finger keeps it");
    let value = styled::slider_value(harness.document(), slider);
    assert!(
        (value - 0.5).abs() < 0.02,
        "the drag went on across both touches, but the slider read {value}"
    );

    harness.quick_tap_trackpad();
    assert!(!harness.simulated_left_held(), "a single tap releases it");
}
