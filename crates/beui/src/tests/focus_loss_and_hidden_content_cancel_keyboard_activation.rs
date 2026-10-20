use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn focus_loss_and_hidden_content_cancel_keyboard_activation() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let button = NodeRef::new();
    let (document, [hidden, _after]) = toolbar_of(move || {
        [
            view! {
                <Frame visible=true>
                    <LabelledButton
                        @node_ref=&button
                        label="Click"
                        on_click={move || clicks.update(|clicks| *clicks += 1)}
                    />
                </Frame>
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    let button = button.get();
    let hidden = kind_of::<FrameNode>(&document, hidden);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get_untracked(), 0);
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    harness.document.set_visible(hidden, false);
    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert_eq!(clicks.get_untracked(), 0);
    assert!(!unstyled::button_focused(harness.document(), button).get());
    assert!(!unstyled::button_active(harness.document(), button).get());
}
