use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn a_simulated_mouse_click_lands_where_the_trackpad_moved_its_cursor() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Trackpad option" checked=false />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    assert!(harness.mouse_simulation());

    let target = harness.center(checkbox);
    harness.point_at(target);
    assert!(harness.simulated_cursor().distance(target) < 1.0);
    assert!(!styled::checkbox_checked(harness.document(), checkbox));

    harness.tap_trackpad();

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
