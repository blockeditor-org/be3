use super::*;
use crate::reactive::view;
use crate::unstyled::{TextArea, TextAreaState, text_area_shown};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_text_area_shows_its_placeholder_until_something_is_typed() {
    let area = NodeRef::new();
    let slot = area.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea @node_ref=&slot state={state} placeholder="Write something" />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        text_area_shown(harness.document(), area.get()),
        "Write something"
    );

    harness.click(pos2(300.0, 16.0));
    harness.type_text("a");

    assert_eq!(text_area_shown(harness.document(), area.get()), "a");
}
