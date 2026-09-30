use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn typing_into_a_focused_text_area_inserts_the_text() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");

    harness.click(pos2(300.0, 16.0));
    harness.type_text(" two");

    assert_eq!(String::from_utf8_lossy(&state.bytes()), "one two");
}
