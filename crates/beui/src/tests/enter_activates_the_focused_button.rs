use super::*;
use crate::reactive::*;

#[test]
fn enter_activates_the_focused_button() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                on_click={move || clicks.update(|clicks| *clicks += 1)}
            />
        }]
    });
    let active = document.api::<unstyled::ButtonApi>(button).active();
    let mut harness = harness(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    assert!(active.get());

    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert!(!active.get());
    assert_eq!(clicks.get_untracked(), 1);
}
