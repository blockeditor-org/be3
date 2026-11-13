use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{ScrollAnchor, TextArea, TextAreaState};
use beui_text_editor_core::TextBuffer;
use std::sync::Arc;

const LINES: usize = 10_000;
const TO: usize = 9_000;

#[test]
fn a_text_scrolled_to_a_far_line_from_cold_settles_in_one_frame() {
    crate::native::fail_unsettled_layout(true);
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let line = |line: usize| format!("line {line} of a long text");
    let anchor = ScrollAnchor {
        line_start: (0..TO).map(|at| line(at).len() + 1).sum(),
        within: 0.0,
    };
    let document = build_node(move || {
        let text = (0..LINES).map(line).collect::<Vec<_>>().join("\n");
        let document =
            Arc::new(TextBuffer::new(text.as_bytes())) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        // Asked before anything is laid out, as a reopened text asks for where it was left.
        state.scroll_to(anchor);
        held.update(|held| *held = Some(state));
        view! {
            <Frame width=500.0 height=600.0>
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
    let frame = harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    assert_eq!(
        state.pending_scroll(),
        None,
        "the scroll reached line {TO} in the frame that started it"
    );
    assert_eq!(state.scrolled().get_untracked(), Some(anchor));
    assert!(
        frame.repaint_after > Duration::ZERO,
        "nothing due now is left for another frame"
    );
    crate::native::fail_unsettled_layout(false);
}
