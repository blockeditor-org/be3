use super::*;
use crate::reactive::view;

#[test]
fn enter_activates_the_focused_button() {
    let _root = crate::reactive::RootScope::new();
    let (clicks, set_clicks) = create_signal(0);
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                on_click={move || set_clicks.update(|clicks| *clicks += 1)}
            />
        }]
    });
    let active = unstyled::button_active(&document, button);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    assert!(active.get());

    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert!(!active.get());
    assert_eq!(clicks.get_untracked(), 1);
}
