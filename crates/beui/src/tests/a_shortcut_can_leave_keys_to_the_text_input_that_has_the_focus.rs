use super::*;
use crate::input::KeyPress;
use crate::reactive::{focus_takes_text, on_shortcut};
use crate::styled::TextInput;

#[component]
fn LetterShortcut(taken: WriteSignal<u32>) -> NodeId {
    on_shortcut(move |press: KeyPress| {
        if press.key != Key::A || !press.pressed || focus_takes_text() {
            return false;
        }
        taken.update(|taken| *taken += 1);
        true
    });
    view! {
        <TextInput value="" />
    }
}

#[test]
fn a_shortcut_can_leave_keys_to_the_text_input_that_has_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let (taken, set_taken) = create_signal(0);
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <LetterShortcut taken={set_taken} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::A, Modifiers::NONE);
    assert_eq!(
        taken.get_untracked(),
        1,
        "nothing takes text, so the shortcut has the key"
    );

    harness.click(harness.center(input));
    harness.key(Key::A, Modifiers::NONE);
    harness.type_text("a");
    assert_eq!(
        taken.get_untracked(),
        1,
        "the focused text input keeps the key"
    );
    assert_eq!(styled::text_input_value(harness.document(), input), "a");
}
