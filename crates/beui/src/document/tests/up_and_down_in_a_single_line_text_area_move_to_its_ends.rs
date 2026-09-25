use super::*;
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn up_and_down_in_a_single_line_text_area_move_to_its_ends() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"one two")) as Arc<dyn text_editor_core::Document>;
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
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(state.caret_indices(), vec![0]);

    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(state.caret_indices(), vec![7]);
}
