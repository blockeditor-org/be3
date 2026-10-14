use super::*;
use crate::reactive::view;
use crate::styled::RadioGroup;

#[test]
fn radio_groups_select_with_space_and_arrows_without_leaving_the_group() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (document, [group, after]) = toolbar_of(|| {
        [
            view! {
                <RadioGroup
                    options={view! {
                        <unstyled::ChoiceOption label="One" />
                        <unstyled::ChoiceOption label="Two" />
                        <unstyled::ChoiceOption label="Three" />
                    }}
                    selected=None
                    on_change={move |value| {
                        set_changes.update(|changes| changes.push(value))
                    }}
                />
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    let after_focus = unstyled::button_focused(&document, after);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        None
    );
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        Some(2)
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(changes.get_untracked(), vec![Some(0), Some(2), Some(0)]);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
