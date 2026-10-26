use accesskit::Role;

use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_read_only_text_area_selects_and_refuses_edits_and_reads_as_read_only() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(b"kept as it is")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} read_only=true />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");

    let field = harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == Role::MultilineTextInput)
        .expect("the text area is absent from the accessibility tree");
    assert!(field.is_read_only(), "a read-only area reads as read-only");
    assert!(!field.is_disabled(), "a read-only area is not disabled");

    harness.click(pos2(300.0, 16.0));
    harness.type_text("typed");
    harness.key(Key::Backspace, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::A, Modifiers::CTRL);
    harness.key(Key::X, Modifiers::CTRL);
    harness.key(Key::Z, Modifiers::CTRL);
    harness.frame(Vec::new());

    assert_eq!(&*state.bytes(), b"kept as it is");
    assert!(
        state
            .selection_ranges()
            .iter()
            .any(|range| !range.is_empty()),
        "read-only text can still be selected"
    );
}
