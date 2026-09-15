use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn holding_the_simulated_left_button_drags_while_another_finger_moves_the_cursor() {
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

    let left = harness.simulated_button(0);
    harness.finger(1, TouchPhase::Start, left);
    harness.point_at(track.center());
    harness.finger(1, TouchPhase::End, left);
    harness.frame(Vec::new());

    let value = styled::slider_value(harness.document(), slider);
    assert!((value - 0.5).abs() < 0.01, "the slider read {value}");
}
