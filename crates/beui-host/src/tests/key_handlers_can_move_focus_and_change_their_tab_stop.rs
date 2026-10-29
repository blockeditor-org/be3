use super::*;
use crate::KeyPress;
use crate::reactive::*;

#[test]
fn key_handlers_can_move_focus_and_change_their_tab_stop() {
    let _root = crate::reactive::RootScope::new();
    let skipped = create_signal(false);
    let second = NodeRef::new();
    let (document, [_first, _second]) = toolbar_of(move || {
        [
            view! {
                <unstyled::Button
                    tab_stop={create_memo(move || !skipped.get())}
                    on_key={move |press: KeyPress| {
                        if press.key != Key::ArrowRight || !press.pressed {
                            return false;
                        }
                        skipped.set(true);
                        crate::native::focus_within(second.get());
                        true
                    }}
                >
                    <ButtonFace label="First" />
                </unstyled::Button>
            },
            view! {
                <LabelledButton @node_ref=&second label="Second" />
            },
        ]
    });
    let flag = document.api::<unstyled::ButtonApi>(second.get()).focused();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert!(flag.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    assert!(flag.get());
}
