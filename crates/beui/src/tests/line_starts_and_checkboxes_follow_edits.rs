use super::*;
use crate::reactive::{Frame, view};
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

type Checkbox = (usize, std::ops::Range<usize>, bool);

fn indexed(state: &TextAreaState) -> (Vec<usize>, Vec<Checkbox>) {
    state.with_snapshot(|snapshot| {
        let checkboxes = snapshot
            .checkboxes
            .iter()
            .map(|checkbox| {
                (
                    checkbox.line_start,
                    checkbox.marker.clone(),
                    checkbox.checked,
                )
            })
            .collect();
        (snapshot.starts.to_vec(), checkboxes)
    })
}

#[test]
fn line_starts_and_checkboxes_follow_edits() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let text = "intro\n- [ ] one\n- [x] two\nplain\n  - [ ] three\nend";
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(text)) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <Frame width=400.0 height=300.0>
                <TextArea state={state} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");

    let edits: [(usize, usize, &[u8]); 6] = [
        (6, 0, b"- [ ] new\n"),
        (0, 0, b"\n\n"),
        (9, 4, b"x"),
        (20, 12, b""),
        (3, 0, b"- [X] \n- "),
        (0, 0, b"- [ ] "),
    ];
    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        for (at, delete, insert) in edits {
            let length = state.bytes().len();
            let start = state.core().position(at.min(length));
            let end = state.core().position((at + delete).min(length));
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
