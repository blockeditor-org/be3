use super::*;
use crate::reactive::{Frame, view};
use crate::styled::{TextArea, text_area_surface};
use crate::unstyled::{TextAreaState, text_area_handles};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn holding_the_caret_handle_below_a_short_text_area_keeps_scrolling() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let area = NodeRef::new();
    let (sink, slot) = (held.clone(), area.clone());
    let document = build(move || {
        let lines = (1..=40)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let document =
            Arc::new(TextBuffer::new(lines.as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <Frame height=120.0>
                <TextArea @node_ref=&slot state={state} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    let surface = text_area_surface(harness.document(), area.get());
    let visible = harness.rect(surface);
    let start = pos2(visible.center().x, visible.top() + 16.0);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = text_area_handles(harness.document(), surface);
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
