use super::*;
use crate::input::{ImeEvent, ImeText};
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn an_ime_correction_replaces_the_word_it_names() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(b"say helo ")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} single_line=true />
        }
    });
    let mut harness = harness(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness
            .frame(Vec::new())
            .window
            .ime
            .and_then(|area| area.text),
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
        after.window.ime.and_then(|area| area.text),
        Some(ImeText {
            start: 0,
            text: "say hello ".to_owned(),
            selection: 9..9,
            composing: None,
        }),
        "the caret ends after the replacement"
    );
}
