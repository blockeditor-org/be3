use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

const TEXT: &str = "- [ ] buy milk";
const MARKER: usize = 5;

#[test]
fn the_caret_steps_evenly_across_a_markdown_checkbox() {
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
    let layout = state.layout().get_untracked();
    let xs: Vec<f32> = (0..=MARKER)
        .map(|byte| {
            layout
                .caret_rect(byte)
                .expect("the byte was laid out")
                .min
                .x
        })
        .collect();
    let step = (xs[MARKER] - xs[0]) / MARKER as f32;
    assert!(step > 0.0, "the checkbox has a width: {xs:?}");
    for (byte, x) in xs.iter().enumerate() {
        let wanted = xs[0] + step * byte as f32;
        assert!(
            (x - wanted).abs() < 0.5,
            "byte {byte} sits at {x}, not {wanted}: {xs:?}"
        );
    }
}
