use accesskit::Role;

use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn ctrl_f_opens_the_find_bar_over_a_text_area() {
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"find me")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::new(document);
    let find_field = |harness: &Harness| {
        harness
            .accessible()
            .iter()
            .any(|node| node.role() == Role::TextInput && node.label() == Some("Find"))
    };
    harness.frame(Vec::new());
    assert!(!find_field(&harness));

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::F, Modifiers::CTRL);
    harness.frame(Vec::new());

    assert!(find_field(&harness));
}
