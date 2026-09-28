use super::*;
use crate::unstyled::{DragHandle, Draggable};

#[test]
fn a_drag_preview_follows_the_pointer_until_the_drop() {
    let (document, [source]) = toolbar_of(|| {
        [view! {
            <Draggable
                payload={1_u32}
                preview={|_: u32| view! {
                    <Frame @test_id="ghost" width=20.0 height=20.0 />
                }}
            >
                {|_: DragHandle| view! {
                    <Frame width=40.0 height=40.0 />
                }}
            </Draggable>
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("ghost").is_none(),
        "nothing is dragged yet"
    );

    let from = harness.center(source);
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    let to = pos2(200.0, 150.0);
    harness.frame(vec![Event::PointerMoved(to)]);
    let output = harness.frame(Vec::new());
    let ghost = output
        .test_id_rect("ghost")
        .expect("the preview shows while dragging");
    assert!(
        (ghost.left() - to.x).abs() < 30.0 && (ghost.top() - to.y).abs() < 30.0,
        "the preview sits beside the pointer, at {ghost:?}"
    );

    harness.frame(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("ghost").is_none(),
        "the preview goes with the drop"
    );
}
