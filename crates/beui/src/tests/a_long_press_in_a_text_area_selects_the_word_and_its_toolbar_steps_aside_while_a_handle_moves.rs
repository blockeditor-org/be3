use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::{TextAreaState, text_area_handles};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_long_press_in_a_text_area_selects_the_word_and_its_toolbar_steps_aside_while_a_handle_moves() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let area = NodeRef::new();
    let (sink, slot) = (held.clone(), area.clone());
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new(b"Hello brave world")) as Arc<dyn text_editor_core::Document>;
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
    let surface = styled::text_area_surface(harness.document(), root);
    let canvas = harness.rect(state.canvas().get());
    let on_hello = pos2(canvas.left() + 12.0, canvas.top() + 16.0);

    harness.touch(TouchPhase::Start, on_hello);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, on_hello);
    harness.frame(Vec::new());
    assert_eq!(
        state.selection_ranges(),
        vec![0..5],
        "the long press selects the word"
    );
    assert!(
        text_within(harness.document(), root, "Copy").is_some(),
        "and shows the toolbar"
    );
    assert!(
        text_within(harness.document(), root, "Select All").is_some(),
        "with the selection's actions on it"
    );

    let handles = text_area_handles(harness.document(), surface);
    let [_, end] = handles[..] else {
        panic!("a word selected by touch shows two handles, not {handles:?}");
    };
    let further = pos2(end.x + 40.0, end.y);
    harness.touch(TouchPhase::Start, end);
    harness.touch(TouchPhase::Move, pos2(end.x + 20.0, end.y));
    harness.touch(TouchPhase::Move, further);
    harness.frame(Vec::new());
    assert!(
        text_within(harness.document(), root, "Copy").is_none(),
        "the toolbar steps aside while a handle moves"
    );
    harness.touch(TouchPhase::End, further);
    harness.frame(Vec::new());
    let range = state.selection_ranges()[0].clone();
    assert!(
        range.start == 0 && range.end > 5,
        "the handle grew the selection to {range:?}"
    );
    assert!(
        text_within(harness.document(), root, "Copy").is_some(),
        "and comes back once the handle is let go"
    );
}
