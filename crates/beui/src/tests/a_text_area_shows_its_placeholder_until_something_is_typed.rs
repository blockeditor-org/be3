use super::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaApi, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_text_area_shows_its_placeholder_until_something_is_typed() {
    let _root = crate::reactive::RootScope::new();
    let area = NodeRef::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea
                colors={area_colors()}
                font_size=AREA_FONT_SIZE
                padding=AREA_PADDING
                @node_ref=&area
                state={state}
                placeholder="Write something"
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().api::<TextAreaApi>(area.get()).shown(),
        "Write something"
    );

    harness.click(pos2(300.0, 16.0));
    harness.type_text("a");

    assert_eq!(
        harness.document().api::<TextAreaApi>(area.get()).shown(),
        "a"
    );
}
