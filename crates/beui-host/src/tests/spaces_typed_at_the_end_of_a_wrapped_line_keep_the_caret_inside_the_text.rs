use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use beui_text_editor_core::{EditorCommand, TextBuffer};
use std::sync::Arc;

const TEXT: &str = "Words enough to wrap onto a second row of this narrow text area";

fn texts(document: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    if document.arena.kind_of::<TextNode>(id).is_some() {
        out.push(id);
    }
    for child in document.children(id) {
        texts(document, child, out);
    }
}

#[test]
fn spaces_typed_at_the_end_of_a_wrapped_line_keep_the_caret_inside_the_text() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <Frame width=300.0 height=400.0>
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    state={state}
                    scroll={plain_scroll()}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(100.0, 20.0));
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    crate::native::with_reactive_scope(harness.document_mut(), || {
        let at = state.with_core(|core| core.position(TEXT.len()));
        state.execute(EditorCommand::SetSelection {
            anchor: at,
            focus: at,
        });
    });
    harness.frame(Vec::new());

    for _ in 0..30 {
        harness.type_text(" ");
    }
    harness.frame(Vec::new());
    let document = harness.document();
    let mut found = Vec::new();
    texts(document, document.root().expect("a root"), &mut found);
    let node = found
        .into_iter()
        .map(|id| kind_of::<TextNode>(document, id))
        .find(|node| document.text(*node).starts_with("Words"))
        .expect("the paragraph is shown");
    let layout = document.text_layout(node).expect("the text was laid out");
    let shown = document.text(node);
    assert!(layout.lines.len() > 1, "the text wraps");
    assert!(shown.ends_with("   "), "the spaces were typed: {shown:?}");
    let width = document
        .node_rect(node)
        .expect("the text was placed")
        .width();
    let caret = layout.caret_rect(shown.len(), 2.0);
    assert!(
        caret.right() <= width + 0.5,
        "the caret after the spaces stays inside the text: {} of {width}",
        caret.right()
    );
    for rect in layout.selection_rects(0..shown.len()) {
        assert!(
            rect.right() <= width + 0.5,
            "a selection over the spaces stops at the text's edge: {} of {width}",
            rect.right()
        );
    }
}
