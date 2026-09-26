use super::*;
use crate::input::ImeEvent;
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn the_keyboard_is_asked_for_at_the_caret_and_after_an_ime_composition() {
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new(b"hello world")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea state={state} single_line=true />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Home, Modifiers::NONE);
    let start = harness
        .frame(Vec::new())
        .ime
        .expect("a focused text area asks for the keyboard");
    assert!(
        start.cursor.width() < start.rect.width() / 2.0,
        "the cursor area is the caret, not the whole field: {start:?}"
    );

    harness.key(Key::End, Modifiers::NONE);
    let end = harness
        .frame(Vec::new())
        .ime
        .expect("the keyboard is still asked for");
    assert!(
        end.cursor.min.x > start.cursor.min.x,
        "the cursor area follows the caret: {start:?} then {end:?}"
    );

    let composing = harness
        .frame(vec![Event::Ime(ImeEvent::Preedit("nihao".to_owned()))])
        .ime
        .expect("the keyboard is asked for while composing");
    assert!(
        composing.cursor.min.x > end.cursor.min.x,
        "the cursor area sits after the composition: {end:?} then {composing:?}"
    );

    let cleared = harness
        .frame(vec![Event::Ime(ImeEvent::Disabled)])
        .ime
        .expect("the keyboard is still asked for");
    assert_eq!(
        cleared.cursor, end.cursor,
        "a cancelled composition leaves the caret where it was"
    );
}
