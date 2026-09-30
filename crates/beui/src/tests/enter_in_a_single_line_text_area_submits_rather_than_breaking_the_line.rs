use super::*;
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn enter_in_a_single_line_text_area_submits_rather_than_breaking_the_line() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let submitted = Rc::new(Cell::new(0));
    let (sink, count) = (held.clone(), submitted.clone());
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <TextArea
                state={state}
                single_line=true
                on_submit={move || count.set(count.get() + 1)}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(vec![Event::Text(" two\nthree".to_owned())]);

    assert_eq!(submitted.get(), 1);
    assert_eq!(String::from_utf8_lossy(&state.bytes()), "one twothree");
}
