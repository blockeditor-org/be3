use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn holding_the_caret_handle_past_the_edge_of_a_narrow_input_keeps_scrolling() {
    let _root = crate::reactive::RootScope::new();
    let input = NodeRef::new();
    let (document, [_sized]) = toolbar_of(move || {
        [view! {
            <Frame width=80.0>
                <TextInput @node_ref=&input value="a value that is much wider than the field" />
            </Frame>
        }]
    });
    let input = input.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .text(),
    );
    let start = pos2(text.left() + 1.0, text.center().y);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .handles(harness.document());
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    let placed = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .caret();
    let past_the_edge = pos2(text.right() + 40.0, handle.y);
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::Move, past_the_edge);
    let reached = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .caret();
    for _ in 0..5 {
        harness.frame(Vec::new());
    }
    let held = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .caret();
    harness.touch(TouchPhase::End, past_the_edge);

    assert!(reached > placed, "dragging the handle moves the caret");
    assert!(
        held >= reached + 3,
        "holding past the edge keeps moving the caret: {reached} then {held}"
    );
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .selection()
            .is_empty()
    );
}
