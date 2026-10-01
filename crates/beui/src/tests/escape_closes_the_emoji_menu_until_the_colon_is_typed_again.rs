use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn escape_closes_the_emoji_menu_until_the_colon_is_typed_again() {
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
    let offered = |harness: &Harness| {
        harness
            .document()
            .find_test_id("text.completion.0")
            .is_some()
    };

    harness.click(pos2(300.0, 16.0));
    harness.type_text(":hea");
    harness.frame(Vec::new());
    assert!(offered(&harness));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(!offered(&harness), "Escape closes the menu");
    harness.type_text("r");
    harness.frame(Vec::new());
    assert!(
        !offered(&harness),
        "typing on after Escape leaves it closed"
    );

    harness.key(Key::Enter, Modifiers::NONE);
    harness.type_text("10:30 and :");
    harness.frame(Vec::new());
    assert!(offered(&harness), "a new colon opens it again");
    harness.key(Key::Backspace, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(!offered(&harness), "deleting the colon closes it");
    assert_eq!(String::from_utf8_lossy(&state.bytes()), ":hear\n10:30 and ");
}
