use super::*;
use crate::reactive::{Frame, NodeRef, view};
use crate::styled::TextInput;

#[test]
fn holding_the_caret_handle_past_the_edge_of_a_narrow_input_keeps_scrolling() {
    let input = NodeRef::new();
    let (document, [_sized]) = toolbar_of({
        let input = input.clone();
        move || {
            [view! {
                <Frame width=80.0>
                    <TextInput @node_ref=&input value="a value that is much wider than the field" />
                </Frame>
            }]
        }
    });
    let input = input.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(unstyled::text_input_text(harness.document(), input));
    let start = pos2(text.left() + 1.0, text.center().y);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = unstyled::text_input_handles(harness.document(), input);
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    let placed = unstyled::text_input_caret(harness.document(), input);
    let past_the_edge = pos2(text.right() + 40.0, handle.y);
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::Move, past_the_edge);
    let reached = unstyled::text_input_caret(harness.document(), input);
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(60));
        harness.frame(Vec::new());
    }
    let held = unstyled::text_input_caret(harness.document(), input);
    harness.touch(TouchPhase::End, past_the_edge);

    assert!(reached > placed, "dragging the handle moves the caret");
    assert!(
        held >= reached + 3,
        "holding past the edge keeps moving the caret: {reached} then {held}"
    );
    assert!(unstyled::text_input_selection(harness.document(), input).is_empty());
}
