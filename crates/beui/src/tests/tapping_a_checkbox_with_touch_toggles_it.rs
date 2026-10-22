use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tapping_a_checkbox_with_touch_toggles_it() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Touch option" checked=false />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let center = harness.center(checkbox);

    harness.touch(TouchPhase::Start, center);
    harness.touch(TouchPhase::End, center);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
}
