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
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                        <unstyled::ChoiceOption value="Three" label="Three" />
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
            .api::<styled::ChoiceApi<&str>>(group)
            .selected(),
        None
    );
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(group)
            .selected(),
        Some("Three")
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        changes.get_untracked(),
        vec![Some("One"), Some("Three"), Some("One")]
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
