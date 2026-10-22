use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn radio_groups_select_with_space_and_arrows_without_leaving_the_group() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [group, after]) = toolbar_of(|| {
        [
            view! {
                <RadioGroup
                    options={view! {
                        <unstyled::ChoiceOption label="One" />
                        <unstyled::ChoiceOption label="Two" />
                        <unstyled::ChoiceOption label="Three" />
                    }}
                    selected={Binding::new(None, move |value| {
                        changes.update(|changes| changes.push(value))
                    })}
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
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(group)
            .selected(),
        None
    );
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(group)
            .selected(),
        Some(2)
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(changes.get_untracked(), vec![Some(0), Some(2), Some(0)]);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
