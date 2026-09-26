use super::*;
use crate::input::ImeEvent;
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn an_ime_composition_pushes_the_text_after_the_caret_along() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new(b"hello world")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <TextArea state={state} single_line=true />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    let x_of = |byte: usize| {
        state
            .layout()
            .get_untracked()
            .caret_rect(byte)
            .expect("every byte of the text has a place")
            .min
            .x
    };
    let before = [1, 5, 11].map(x_of);
    let first = x_of(0);

    harness.frame(vec![Event::Ime(ImeEvent::Preedit("xyz".to_owned()))]);
    let composing = [1, 5, 11].map(x_of);
    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "hello world",
        "the composition is shown, not written"
    );
    for (was, is) in before.iter().zip(composing) {
        assert!(
            is > *was,
            "the text after the caret moves along for the composition: {before:?} then {composing:?}"
        );
    }
    assert_eq!(
        x_of(0),
        first,
        "the text before the caret stays where it was"
    );

    harness.frame(vec![Event::Ime(ImeEvent::Disabled)]);
    assert_eq!(
        [1, 5, 11].map(x_of),
        before,
        "a cancelled composition gives the text its place back"
    );
}
