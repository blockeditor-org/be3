use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

const TEXT: &str = "- a\n    - b";
const LINE_END: usize = 3;
const NESTED: usize = 4;
const TEXT_START: usize = 10;

#[test]
fn the_caret_steps_through_a_markdown_list_items_hidden_indentation_and_marker() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let canvas = harness.rect(state.canvas().get());
    let layout = state.layout().get_untracked();
    let caret = |byte: usize| {
        layout
            .caret_rect(byte)
            .expect("the byte was laid out")
            .translate(canvas.min.to_vec2())
    };

    let xs: Vec<f32> = (NESTED..=TEXT_START)
        .map(|byte| caret(byte).min.x)
        .collect();
    let step = (xs[xs.len() - 1] - xs[0]) / (xs.len() - 1) as f32;
    assert!(
        step > 0.0,
        "the indentation and marker have a width: {xs:?}"
    );
    for (index, x) in xs.iter().enumerate() {
        let wanted = xs[0] + step * index as f32;
        assert!(
            (x - wanted).abs() < 0.5,
            "byte {} sits at {x}, not {wanted}: {xs:?}",
            NESTED + index
        );
    }
    assert!(
        caret(TEXT_START).min.x > caret(2).min.x,
        "the nested item's text is indented past its parent's"
    );

    let end = caret(LINE_END);
    harness.click(pos2(end.min.x + 0.25, end.center().y));
    assert_eq!(state.caret_indices(), vec![LINE_END]);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        state.caret_indices(),
        vec![NESTED],
        "Right moves to the nested line's start"
    );
    let mut visited = Vec::new();
    let mut x = caret(NESTED).min.x;
    while state.caret_indices() != vec![TEXT_START] && visited.len() < TEXT.len() {
        harness.key(Key::ArrowRight, Modifiers::NONE);
        let at = state.caret_indices()[0];
        assert!(caret(at).min.x > x, "the caret moves right at byte {at}");
        x = caret(at).min.x;
        visited.push(at);
    }
    assert_eq!(
        visited,
        vec![6, 8, 9, TEXT_START],
        "Right steps through the indentation a soft tab at a time, then the marker's bytes"
    );
}
