use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn focus_loss_and_hidden_content_cancel_keyboard_activation() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let shown = create_signal(true);
    let button = NodeRef::new();
    let (document, _) = toolbar_of(move || {
        [
            view! {
                <List spacing=0.0>
                    <ShowKeepAlive condition={shown}>
                        <LabelledButton
                            @node_ref=&button
                            label="Click"
                            on_click={move || clicks.update(|clicks| *clicks += 1)}
                        />
                    </ShowKeepAlive>
                </List>
            },
            view! {
                <LabelledButton label="After" />
            },
        ]
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true)]);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false)]);
    assert_eq!(clicks.get_untracked(), 0);
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.frame(vec![key_event(Key::Enter, true)]);
    with_installed(harness.document_mut(), |_| shown.set(false));
    harness.frame(vec![key_event(Key::Enter, false)]);
    assert_eq!(clicks.get_untracked(), 0);
    assert_ne!(harness.document().focused_node(), Some(button));
    with_installed(harness.document_mut(), |_| shown.set(true));
    harness.frame(Vec::new());
    assert!(
        !harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .focused()
            .get()
    );
    assert!(
        !harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .active()
            .get()
    );
}
