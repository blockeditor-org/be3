use super::*;
use crate::reactive::*;
use crate::styled::*;

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
    assert!(
        !harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );

    harness.tap_trackpad();

    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
}
