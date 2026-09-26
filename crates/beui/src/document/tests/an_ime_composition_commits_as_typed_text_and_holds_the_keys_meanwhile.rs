use super::*;
use crate::input::ImeEvent;
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn an_ime_composition_commits_as_typed_text_and_holds_the_keys_meanwhile() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"ab")) as Arc<dyn text_editor_core::Document>;
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

    harness.frame(vec![
        Event::Ime(ImeEvent::Enabled),
        Event::Ime(ImeEvent::Preedit("ni".to_owned())),
    ]);
    harness.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "ab",
        "a composition is not text yet, and the keys that edit it are the input method's"
    );

    harness.frame(vec![
        Event::Ime(ImeEvent::Preedit(String::new())),
        Event::Ime(ImeEvent::Commit("你".to_owned())),
    ]);
    assert_eq!(String::from_utf8_lossy(&state.bytes()), "ab你");

    harness.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "ab",
        "keys edit the text again once the composition is committed"
    );
}
