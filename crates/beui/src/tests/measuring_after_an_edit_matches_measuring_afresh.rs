use super::*;
use crate::reactive::{Frame, view};
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

fn text() -> String {
    (0..300)
        .map(|line| match line % 10 {
            0 => format!("# Heading {line}"),
            _ => format!("line {line} has *some* words that wrap at a narrow width, more or less"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn measuring_after_an_edit_matches_measuring_afresh() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new(text().as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <Frame width=500.0 height=300.0>
                <TextArea state={state} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");

    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        let before = state.measure(&[], 300.0).expect("the text was measured");
        let at = state.core().position(text().find("line 21 ").unwrap());
        state.execute(EditorCommand::SetSelection {
            anchor: at,
            focus: at,
        });
        state.execute(EditorCommand::InsertText(b"```\n"));
        let after = state
            .measure(&[], 300.0)
            .expect("the edited text was measured");

        let edited = state.bytes().to_vec();
        let fresh = TextAreaState::new(Arc::new(TextBuffer::new(&edited)));
        let expected = fresh
            .measure(&[], 300.0)
            .expect("the fresh text was measured");
        assert_ne!(
            before, after,
            "opening a fence below the top restyles what follows"
        );
        assert_eq!(after, expected);
    });
}
