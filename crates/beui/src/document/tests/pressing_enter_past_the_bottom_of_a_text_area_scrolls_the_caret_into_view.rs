use super::*;
use crate::reactive::{Frame, view};
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn pressing_enter_past_the_bottom_of_a_text_area_scrolls_the_caret_into_view() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let area = NodeRef::new();
    let sink = held.clone();
    let slot = area.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"top")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        sink.replace(Some(state.clone()));
        view! {
            <Frame height=120.0>
                <TextArea @node_ref=&slot state={state} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    let visible = harness.rect(area.get());

    harness.click(pos2(300.0, 16.0));
    for _ in 0..20 {
        harness.key(Key::Enter, Modifiers::NONE);
    }
    harness.type_text("bottom");
    harness.frame(Vec::new());

    let canvas = harness.rect(state.canvas().get());
    let caret = *state.caret_indices().first().expect("the text area has a caret");
    let rect = state
        .layout()
        .get_untracked()
        .caret_rect(caret)
        .expect("the caret was laid out")
        .translate(canvas.min.to_vec2());
    assert!(
        visible.contains_rect(rect),
        "the caret {rect:?} is inside the text area {visible:?}"
    );
}
