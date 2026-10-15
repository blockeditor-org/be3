use super::*;
use crate::reactive::*;

#[test]
fn activation_requires_a_matching_release_and_escape_cancels_it() {
    let _root = crate::reactive::RootScope::new();
    let (clicks, set_clicks) = create_signal(0);
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click"
                on_click={move || set_clicks.update(|clicks| *clicks += 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert_eq!(clicks.get_untracked(), 0);
    assert!(unstyled::button_active(harness.document(), button).get());
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get_untracked(), 0);
    assert!(!unstyled::button_active(harness.document(), button).get());
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(clicks.get_untracked(), 1);
}
