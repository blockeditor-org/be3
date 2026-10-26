use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

const LINES: usize = 20_000;
const SHOWN: usize = 64;

fn built_lines(document: &Document, node: NodeId) -> Option<usize> {
    if document.node_kind(node) == "virtual list" {
        return Some(document.children(node).len());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| built_lines(document, child))
}

#[test]
fn a_long_text_area_only_builds_the_lines_in_view() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let area = NodeRef::new();
    let document = build_node(move || {
        let text = (0..LINES)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let document =
            Arc::new(TextBuffer::new(text.as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <Frame @node_ref=&area width=500.0 height=300.0>
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    state={state}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let root = harness.document().root().expect("the document has a root");
    let built = built_lines(harness.document(), root).expect("the lines are a virtual list");
    assert!(
        built < SHOWN,
        "only the lines in view are built, not {built}"
    );

    harness.click(pos2(250.0, 20.0));
    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        let end = state.with_core(|core| core.position(state.bytes().len()));
        state.execute(EditorCommand::SetSelection {
            anchor: end,
            focus: end,
        });
        state.reveal_cursor();
    });
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    let visible = harness.rect(area.get());
    let canvas = harness.rect(state.canvas().get());
    let caret = *state.caret_indices().first().expect("the area has a caret");
    let rect = state
        .layout()
        .get_untracked()
        .caret_rect(caret)
        .expect("the caret was laid out")
        .translate(canvas.min.to_vec2());
    assert_eq!(caret, state.bytes().len());
    assert!(
        visible.contains_rect(rect),
        "jumping to the end scrolls the last line into view: {rect:?} in {visible:?}"
    );
    let built = built_lines(harness.document(), root).expect("the lines are a virtual list");
    assert!(
        built < SHOWN,
        "scrolling keeps building only what is in view, not {built}"
    );
}
