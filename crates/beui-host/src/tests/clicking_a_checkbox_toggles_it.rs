use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn clicking_a_checkbox_toggles_it() {
    let _root = crate::reactive::RootScope::new();
    let checked = create_signal(false);
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Show timings" checked={checked} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
    assert!(checked.get_untracked());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(
        !harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
    assert!(!checked.get_untracked());
}
