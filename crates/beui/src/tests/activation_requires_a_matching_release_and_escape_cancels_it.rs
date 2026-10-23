use super::*;
use crate::reactive::*;

#[test]
fn activation_requires_a_matching_release_and_escape_cancels_it() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <LabelledButton label="Click" on_click={move || clicks.update(|clicks| *clicks += 1)} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true)]);
    harness.frame(vec![key_event(Key::Enter, false)]);
    assert_eq!(clicks.get_untracked(), 0);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .active()
            .get()
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false)]);
    assert_eq!(clicks.get_untracked(), 0);
    assert!(
        !harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .active()
            .get()
    );
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(clicks.get_untracked(), 1);
}
