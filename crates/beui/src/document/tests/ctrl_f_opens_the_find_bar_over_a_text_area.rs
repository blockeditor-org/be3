use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn ctrl_f_opens_the_find_bar_over_a_text_area() {
    let area = NodeRef::new();
    let slot = area.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"find me")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea @node_ref=&slot state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let root = area.get();
    assert!(text_within(harness.document(), root, "Find").is_none());

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::F, Modifiers::CTRL);
    harness.frame(Vec::new());

    assert!(text_within(harness.document(), root, "Find").is_some());
}
