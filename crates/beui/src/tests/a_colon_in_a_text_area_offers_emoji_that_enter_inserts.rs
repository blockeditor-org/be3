use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_colon_in_a_text_area_offers_emoji_that_enter_inserts() {
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
    let offered = |harness: &Harness| harness.document().find_test_id("text.emoji.0").is_some();

    harness.click(pos2(300.0, 16.0));
    harness.type_text("I am ");
    harness.frame(Vec::new());
    assert!(!offered(&harness), "no menu before a colon");

    harness.type_text(":");
    harness.frame(Vec::new());
    assert!(offered(&harness), "a colon after a space opens the menu");

    harness.type_text("rocke");
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "I am \u{1f680}",
        "Enter replaces the colon and what was typed after it with the emoji"
    );
    assert!(
        !offered(&harness),
        "the menu closes once an emoji is picked"
    );
}
