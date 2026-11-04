use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn listbox_typeahead_matches_prefixes_and_cycles_repeated_letters() {
    let (document, [listbox, after]) = toolbar_of(|| {
        [
            view! {
                <Listbox
                    options={view! {
                        <unstyled::ChoiceOption label="Apple" />
                        <unstyled::ChoiceOption label="Banana" />
                        <unstyled::ChoiceOption label="Blueberry" />
                        <unstyled::ChoiceOption label="Cherry" />
                    }}
                    selected=Some(0)
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
    harness.type_text("bl");
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(listbox)
            .selected(),
        Some(2)
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.type_text("b");
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(listbox)
            .selected(),
        Some(1)
    );
    harness.type_text("b");
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(listbox)
            .selected(),
        Some(2)
    );
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(listbox)
            .selected(),
        Some(3)
    );
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(listbox)
            .selected(),
        Some(0)
    );
}
