use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn space_toggles_checkboxes_switches_and_toggle_buttons() {
    let (document, [checkbox, switch, toggle]) = toolbar_of(|| {
        [
            view! {
                <Checkbox label="Check" checked=false />
            },
            view! {
                <Switch on=false />
            },
            view! {
                <ToggleButton label="Bold" pressed=false />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(switch)
            .checked()
            .get()
    );
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(toggle)
            .checked()
            .get()
    );
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(
        !harness
            .document()
            .api::<styled::ToggleApi>(toggle)
            .checked()
            .get()
    );
}
