use super::*;
use crate::reactive::*;
use crate::styled::*;
use beui::KeyPress;

#[component]
fn LetterShortcut(taken: Signal<u32>) -> NodeId {
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
    let taken = create_signal(0);
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <LetterShortcut taken={taken} />
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
    assert_eq!(
        taken.get_untracked(),
        1,
        "the focused text input keeps the key"
    );
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "a"
    );
}
