use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn empty_choices_and_invalid_selection_do_not_break_tab_navigation() {
    let (document, [empty, tabs, after]) = toolbar_of(|| {
        [
            view! {
                <Listbox
                    options={view! {}}
                    selected=Some("Four")
                />
            },
            view! {
                <Tabs
                    options={view! {
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                    }}
                    selected="Missing"
                />
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    assert_eq!(
        document.api::<styled::ChoiceApi<&str>>(tabs).selected(),
        None
    );
    assert_eq!(
        document.api::<styled::ChoiceApi<&str>>(empty).selected(),
        None
    );
    let after_focus = document.api::<unstyled::ButtonApi>(after).focused();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
