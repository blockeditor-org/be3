use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

const TEXT: &str = "a first line that is long\nshort\nthe third line, longer again";

#[test]
fn arrow_keys_move_a_text_area_caret_the_same_when_measurement_comes_late() {
    let (prompt, late) = prompt_and_late(|| {
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
        harness.settle();
        let state = held.get_untracked().expect("the text area was built");
        let canvas = harness.rect(state.canvas().get());
        harness.click(pos2(canvas.min.x + 150.0, canvas.min.y + 12.0));
        harness.settle();
        let mut carets = vec![state.caret_indices()];
        for key in [
            Key::ArrowDown,
            Key::ArrowDown,
            Key::ArrowUp,
            Key::End,
            Key::ArrowUp,
        ] {
            harness.key(key, Modifiers::default());
            harness.settle();
            carets.push(state.caret_indices());
        }
        carets
    });
    assert!(
        prompt.windows(2).any(|pair| pair[0] != pair[1]),
        "the keys moved the caret: {prompt:?}"
    );
    assert_eq!(
        late, prompt,
        "late measurement moves the caret where prompt measurement does"
    );
}
