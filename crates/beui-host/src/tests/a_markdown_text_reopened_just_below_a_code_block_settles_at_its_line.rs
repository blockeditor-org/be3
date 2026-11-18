use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{ScrollAnchor, TextArea, TextAreaState};
use beui_text_editor_core::{EditorCommand, TextBuffer, TextLanguage};
use std::sync::Arc;

const LINES: usize = 400;
const CODE: std::ops::Range<usize> = 300..316;
const TO: usize = 312;

/// A long Markdown text whose lines are each a row of the estimate's height, but for a fenced
/// code block of lines long enough to wrap.
fn text() -> String {
    (0..LINES)
        .map(|line| match line {
            _ if line == CODE.start => "```rust".to_owned(),
            _ if line + 1 == CODE.end => "```".to_owned(),
            _ if CODE.contains(&line) => format!(
                "let line_{line} = {line}; // a code line long enough to wrap at the width of the view"
            ),
            _ => format!("line {line} of a long text"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_markdown_text_reopened_just_below_a_code_block_settles_at_its_line() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let text = text();
    let anchor = ScrollAnchor {
        line_start: text.split('\n').take(TO).map(|line| line.len() + 1).sum(),
        within: 0.0,
    };
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(text.as_bytes())) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
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
                    scroll={plain_scroll()}
                />
            </Frame>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    // The rows the first layout there measures (the code block's, taller than the estimate) move
    // every row above them. The list places them where they settle, so the offset it shifts to
    // keep them in place is the one the text reads back, and the line asked for stays on top.
    assert_eq!(state.pending_scroll(), None, "the scroll reached its line");
    assert_eq!(state.scrolled().get_untracked(), Some(anchor));
    let canvas = harness.rect(state.canvas().get());
    let caret = state
        .layout()
        .get_untracked()
        .caret_rect(anchor.line_start)
        .expect("the line asked for was laid out");
    // The caret's rect is the canvas's own, and the view starts where the window does.
    assert!(
        (caret.min.y + canvas.min.y).abs() < 1.0,
        "line {TO} is at the top of the view: {} against {}",
        caret.min.y,
        -canvas.min.y
    );
    for _ in 0..3 {
        harness.frame(Vec::new());
        assert_eq!(
            harness.rect(state.canvas().get()),
            canvas,
            "the content keeps its length and offset once there, so the thumb holds still"
        );
    }
}
