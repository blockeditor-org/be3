use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn a_quick_flick_on_the_simulated_trackpad_moves_the_cursor_without_clicking() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Trackpad option" checked=false />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    let target = harness.center(checkbox);
    harness.point_at(target);

    let at = harness.simulated_trackpad();
    let flick = at + Vec2::new(6.0, 0.0);
    harness.finger(1, TouchPhase::Start, at);
    harness.finger(1, TouchPhase::Move, flick);
    harness.finger(1, TouchPhase::End, flick);
    harness.frame(Vec::new());
    assert!(harness.simulated_cursor().distance(target) > 5.0);
    assert!(!styled::checkbox_checked(harness.document(), checkbox));

    harness.tap_trackpad();

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
