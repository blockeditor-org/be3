use super::*;
use crate::reactive::view;
use crate::styled::{TextArea, search_emoji};
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn arrows_pick_which_emoji_the_menu_inserts() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    let offered = search_emoji("smile");
    assert!(offered.len() > 2, "several emoji match \"smile\"");
    assert_eq!(offered[0].label, ":smile:", "an exact name comes first");

    harness.click(pos2(300.0, 16.0));
    harness.type_text(":smile");
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        offered[1].insert,
        "down, down and up land on the second emoji"
    );
    assert_eq!(state.caret_indices(), vec![offered[1].insert.len()]);
}
