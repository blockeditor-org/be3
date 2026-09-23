use super::*;
use crate::input::KeyPress;
use crate::reactive::{focus_takes_text, on_shortcut};
use crate::styled::TextInput;

#[component]
fn LetterShortcut(taken: Rc<Cell<u32>>) -> NodeId {
    on_shortcut(move |press: KeyPress| {
        if press.key != Key::A || !press.pressed || focus_takes_text() {
            return false;
        }
        taken.set(taken.get() + 1);
        true
    });
    view! {
        <TextInput value="" />
    }
}

#[test]
fn a_shortcut_can_leave_keys_to_the_text_input_that_has_the_focus() {
    let taken = Rc::new(Cell::new(0));
    let shortcut = Rc::clone(&taken);
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <LetterShortcut taken={shortcut} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::A, Modifiers::NONE);
    assert_eq!(
        taken.get(),
        1,
        "nothing takes text, so the shortcut has the key"
    );

    harness.click(harness.center(input));
    harness.key(Key::A, Modifiers::NONE);
    harness.type_text("a");
    assert_eq!(taken.get(), 1, "the focused text input keeps the key");
    assert_eq!(styled::text_input_value(harness.document(), input), "a");
}
