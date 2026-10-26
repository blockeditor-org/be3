use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn typing_past_the_end_of_a_narrow_single_line_text_area_keeps_the_caret_in_view() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let area = NodeRef::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <List spacing=0.0>
                <Frame width=80.0>
                    <TextArea
                        colors={area_colors()}
                        look={area_look()}
                        font_size=AREA_FONT_SIZE
                        padding=AREA_PADDING
                        @node_ref=&area
                        state={state}
                        single_line=true
                    />
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a value that is much wider than the field");
    harness.frame(Vec::new());

    let visible = harness.rect(area.get());
    let canvas = harness.rect(state.canvas().get());
    let caret = *state
        .caret_indices()
        .first()
        .expect("the field has a caret");
    let rect = state
        .layout()
        .get_untracked()
        .caret_rect(caret)
        .expect("the caret was laid out")
        .translate(canvas.min.to_vec2());
    assert!(
        visible.contains_rect(rect),
        "the caret {rect:?} is inside the field {visible:?}"
    );
    assert!(visible.height() < 60.0, "the field is one line tall");
}
