use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tabs_have_one_tab_stop_and_wrap_with_arrow_keys() {
    let (document, [_before, tabs, after]) = toolbar_of(|| {
        [
            view! {
                <LabelledButton label="Before" />
            },
            view! {
                <Tabs
                    options={view! {
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                        <unstyled::ChoiceOption value="Three" label="Three" />
                    }}
                    selected="Two"
                />
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    let after_focus = document.api::<unstyled::ButtonApi>(after).focused();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(tabs)
            .selected(),
        Some("Three")
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(tabs)
            .selected(),
        Some("One")
    );
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(tabs)
            .selected(),
        Some("Three")
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(tabs)
            .selected(),
        Some("Three")
    );
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(tabs)
            .selected(),
        Some("One")
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(tabs)
            .selected(),
        Some("Three")
    );
}
