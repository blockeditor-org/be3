use super::*;
use crate::reactive::*;

#[test]
fn losing_window_focus_cancels_a_held_activation_key() {
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
    harness.frame(vec![Input::Focus(false)]);
    assert!(
        !harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .active()
            .get()
    );
    harness.frame(vec![Input::Focus(true), key_event(Key::Space, false)]);
    assert_eq!(clicks.get_untracked(), 0);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(clicks.get_untracked(), 1);
}
