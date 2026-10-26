use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

#[test]
fn moving_the_caret_builds_no_rows_again() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let text = (0..20)
            .map(|line| format!("line {line} with  spaces"))
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
    let built = state.rows_built();
    assert!(built > 0, "the rows in view were built");

    let select = |harness: &mut Harness, anchor: usize, focus: usize| {
        crate::reactive::with_reactive_scope(harness.document_mut(), || {
            let (anchor, focus) =
                state.with_core(|core| (core.position(anchor), core.position(focus)));
            state.execute(EditorCommand::SetSelection { anchor, focus });
        });
        harness.frame(Vec::new());
    };
    for byte in [3, 40, 80, 200] {
        select(&mut harness, byte, byte);
    }
    select(&mut harness, 10, 300);
    select(&mut harness, 0, 0);
    assert_eq!(
        state.rows_built(),
        built,
        "moving the caret or the selection builds no row again"
    );

    harness.type_text("a");
    assert!(
        state.rows_built() > built,
        "an edit builds the rows it changes again"
    );
}
