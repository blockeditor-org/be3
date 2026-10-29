use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextAreaApi, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn holding_the_caret_handle_below_a_short_text_area_keeps_scrolling() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let area = NodeRef::new();
    let document = build_node(move || {
        let lines = (1..=40)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let document =
            Arc::new(TextBuffer::new(lines.as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <Frame height=120.0>
                <TextArea @node_ref=&area state={state} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let surface = harness
        .document()
        .api::<TextAreaSurfaceApi>(area.get())
        .surface();
    let visible = harness.rect(surface);
    let start = pos2(visible.center().x, visible.top() + 16.0);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = harness
        .document()
        .api::<TextAreaApi>(surface)
        .handles(harness.document());
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    let line_of = |state: &TextAreaState| {
        let caret = state.caret_indices()[0];
        state.bytes()[..caret]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
    };
    let placed = line_of(&state);
    let below = pos2(handle.x, visible.bottom() + 40.0);
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::Move, below);
    let reached = line_of(&state);
    for _ in 0..5 {
        harness.frame(Vec::new());
    }
    let held_line = line_of(&state);
    harness.touch(TouchPhase::End, below);

    assert!(reached > placed, "dragging the handle moves the caret");
    assert!(
        held_line >= reached + 3,
        "holding below the edge keeps moving the caret: line {reached} then {held_line}"
    );
    assert!(
        state
            .selection_ranges()
            .iter()
            .all(|range| range.is_empty()),
        "dragging the caret handle does not select"
    );
}
