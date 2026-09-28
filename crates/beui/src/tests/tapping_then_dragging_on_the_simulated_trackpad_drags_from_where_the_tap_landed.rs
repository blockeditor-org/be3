use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn tapping_then_dragging_on_the_simulated_trackpad_drags_from_where_the_tap_landed() {
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
    harness.tap_trackpad();

    let at = harness.simulated_trackpad();
    let delta = track.center() - harness.simulated_cursor();
    harness.finger(1, TouchPhase::Start, at);
    harness.finger(1, TouchPhase::Move, at + delta);
    harness.finger(1, TouchPhase::End, at + delta);
    harness.frame(Vec::new());

    let value = styled::slider_value(harness.document(), slider);
    assert!((value - 0.5).abs() < 0.01, "the slider read {value}");
}
