use super::*;
use crate::input::{ImeEvent, ImeText};
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn an_ime_correction_replaces_the_word_it_names() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new(b"say helo ")) as Arc<dyn text_editor_core::Document>;
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
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness.frame(Vec::new()).ime.and_then(|area| area.text),
        Some(ImeText {
            start: 0,
            text: "say helo ".to_owned(),
            selection: 9..9,
            composing: None,
        }),
        "the input method is told the text around the caret"
    );

    let after = harness.frame(vec![Event::Ime(ImeEvent::ReplaceText {
        range: 4..8,
        text: "hello".to_owned(),
    })]);

    assert_eq!(String::from_utf8_lossy(&state.bytes()), "say hello ");
    assert_eq!(
        after.ime.and_then(|area| area.text),
        Some(ImeText {
            start: 0,
            text: "say hello ".to_owned(),
            selection: 9..9,
            composing: None,
        }),
        "the caret ends after the replacement"
    );
}
