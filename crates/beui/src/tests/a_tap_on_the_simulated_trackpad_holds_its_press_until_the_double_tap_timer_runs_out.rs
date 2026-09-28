use super::*;
use crate::mouse_simulation::DOUBLE_TAP_TIME;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn a_tap_on_the_simulated_trackpad_holds_its_press_until_the_double_tap_timer_runs_out() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Trackpad option" checked=false />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    harness.point_at(harness.center(checkbox));

    let at = harness.simulated_trackpad();
    harness.finger(1, TouchPhase::Start, at);
    harness.finger(1, TouchPhase::End, at);
    let output = harness.frame(Vec::new());
    assert!(harness.simulated_left_held(), "the tap holds the button down");
    assert!(
        output.repaint_after <= DOUBLE_TAP_TIME,
        "the simulation asks to be woken when the timer runs out"
    );
    assert!(!styled::checkbox_checked(harness.document(), checkbox));

    harness.wait_out_double_tap();

    assert!(!harness.simulated_left_held(), "the timer lets it go");
    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
