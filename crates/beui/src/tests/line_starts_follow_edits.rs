use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

fn indexed(state: &TextAreaState) -> Vec<usize> {
    state.with_snapshot(|snapshot| snapshot.starts.to_vec())
}

#[test]
fn line_starts_follow_edits() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let text = "intro\n- [ ] one\n- [x] two\nplain\n  - [ ] three\nend";
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(text)) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <Frame width=400.0 height=300.0>
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

    let edits: [(usize, usize, &[u8]); 6] = [
        (6, 0, b"- [ ] new\n"),
        (0, 0, b"\n\n"),
        (9, 4, b"x"),
        (20, 12, b""),
        (3, 0, b"- [X] \n- "),
        (0, 0, b"- [ ] "),
    ];
    crate::native::with_reactive_scope(harness.document_mut(), || {
        for (at, delete, insert) in edits {
            let length = state.bytes().len();
            let start = state.with_core(|core| core.position(at.min(length)));
            let end = state.with_core(|core| core.position((at + delete).min(length)));
            state.execute(EditorCommand::SetSelection {
                anchor: start,
                focus: end,
            });
            state.execute(EditorCommand::InsertText(insert));
            let edited = state.bytes().to_vec();
            let fresh = TextAreaState::new(Arc::new(TextBuffer::new(&edited)));
            assert_eq!(
                indexed(&state),
                indexed(&fresh),
                "after an edit at {at}: {:?}",
                String::from_utf8_lossy(&edited)
            );
        }
    });
}
