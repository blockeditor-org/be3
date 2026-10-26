use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

#[test]
fn typing_on_a_line_rebuilds_only_that_line() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let text = (0..20)
            .map(|line| format!("line {line} of the text"))
            .collect::<Vec<_>>()
            .join("\n");
        let document =
            Arc::new(TextBuffer::new(text.as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
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
    harness.frame(Vec::new());
    harness.click(pos2(250.0, 20.0));
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        let at = state.with_core(|core| core.position(2));
        state.execute(EditorCommand::SetSelection {
            anchor: at,
            focus: at,
        });
    });
    harness.frame(Vec::new());

    for text in ["a", "b", "c"] {
        let built = state.rows_built();
        harness.type_text(text);
        assert_eq!(
            state.rows_built() - built,
            1,
            "typing {text:?} on the first line builds only that line again"
        );
    }

    let built = state.rows_built();
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(
        state.rows_built() - built <= 2,
        "a new line builds the two lines it split, not the lines below it that moved"
    );
}
