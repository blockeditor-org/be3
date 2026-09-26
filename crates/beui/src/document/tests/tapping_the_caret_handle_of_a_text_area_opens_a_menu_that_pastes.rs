use super::*;
use crate::reactive::view;
use crate::styled::{TextArea, text_area_surface};
use crate::unstyled::{TextAreaState, text_area_handles};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn tapping_the_caret_handle_of_a_text_area_opens_a_menu_that_pastes() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let area = NodeRef::new();
    let (sink, slot) = (held.clone(), area.clone());
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"Hello")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <TextArea @node_ref=&slot state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    let root = area.get();
    let surface = text_area_surface(harness.document(), root);
    let canvas = harness.rect(state.canvas().get());
    let start = pos2(canvas.left() + 1.0, canvas.top() + 16.0);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = text_area_handles(harness.document(), surface);
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::End, handle);
    harness.frame(Vec::new());

    assert!(text_within(harness.document(), root, "Copy").is_none());
    let paste =
        text_within(harness.document(), root, "Paste").expect("tapping the handle opens its menu");
    let row = harness.center(paste);
    harness.touch(TouchPhase::Start, row);
    let output = harness.frame(vec![Event::Touch {
        id: TouchId {
            device: 1,
            finger: 1,
        },
        phase: TouchPhase::End,
        pos: row,
        force: None,
    }]);
    assert!(output.paste_requested);

    harness.frame(vec![Event::Text("Say ".to_owned())]);
    assert_eq!(String::from_utf8_lossy(&state.bytes()), "Say Hello");
}
