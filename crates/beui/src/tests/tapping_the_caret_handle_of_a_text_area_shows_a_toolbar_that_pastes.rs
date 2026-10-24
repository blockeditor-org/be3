use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextAreaApi, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn tapping_the_caret_handle_of_a_text_area_shows_a_toolbar_that_pastes() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let area = NodeRef::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"Hello")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea @node_ref=&area state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let root = area.get();
    let surface = harness.document().api::<TextAreaSurfaceApi>(root).surface();
    let canvas = harness.rect(state.canvas().get());
    let start = pos2(canvas.left() + 1.0, canvas.top() + 16.0);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = harness
        .document()
        .api::<TextAreaApi>(surface)
        .handles(harness.document());
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::End, handle);
    harness.frame(Vec::new());

    assert!(text_within(harness.document(), root, "Copy").is_none());
    let paste = text_within(harness.document(), root, "Paste")
        .expect("tapping the handle shows its toolbar");
    let row = harness.center(paste);
    harness.touch(TouchPhase::Start, row);
    let output = harness.touch(TouchPhase::End, row);
    assert_eq!(output.paste, Some(crate::PasteRequest::Asked));
    assert_eq!(
        output.main().window.requests.pastes,
        [crate::PasteKind::Text]
    );

    harness.frame(vec![Input::Paste(crate::Pasted::Text("Say ".to_owned()))]);
    assert_eq!(String::from_utf8_lossy(&state.bytes()), "Say Hello");
}
