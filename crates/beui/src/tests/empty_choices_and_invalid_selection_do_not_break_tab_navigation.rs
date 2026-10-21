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
                    selected=Some(4)
                />
            },
            view! {
                <Tabs
                    options={view! {
                        <unstyled::ChoiceOption label="One" />
                        <unstyled::ChoiceOption label="Two" />
                    }}
                    selected=99
                />
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    assert_eq!(styled::tabs_selected(&document, tabs), 1);
    assert_eq!(styled::listbox_selected(&document, empty), None);
    let after_focus = unstyled::button_state(&document, after);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get().focused);
}
