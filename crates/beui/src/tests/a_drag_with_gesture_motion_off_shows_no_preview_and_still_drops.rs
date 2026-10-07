use super::*;
use crate::Motion;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn a_drag_with_gesture_motion_off_shows_no_preview_and_still_drops() {
    let dropped = Rc::new(Cell::new(None::<u32>));
    let hovered = Rc::new(Cell::new(0));
    let received = Rc::clone(&dropped);
    let over = Rc::clone(&hovered);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable
                    payload={7_u32}
                    preview={|_: u32| view! {
                        <Frame @test_id="ghost" width=20.0 height=20.0 />
                    }}
                >
                    {|_: DragHandle| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                </Draggable>
            },
            view! {
                <DropTarget
                    on_over={move |at: Option<(u32, DragPoint)>| {
                        if at.is_some() {
                            over.set(over.get() + 1);
                        }
                    }}
                    on_drop={move |(payload, _): (u32, DragPoint)| received.set(Some(payload))}
                >
                    {|_: DropHandle| view! {
                        <Frame width=100.0 height=100.0 />
                    }}
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.context().set_motion(Motion::Still);
    harness.frame(Vec::new());

    let (from, to) = (harness.center(source), harness.center(target));
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(to)]);
    let output = harness.frame(Vec::new());

    assert!(
        output.test_id_rect("ghost").is_none(),
        "no preview follows the pointer"
    );
    assert_eq!(
        hovered.get(),
        0,
        "the target is not told while the drag is held"
    );

    harness.frame(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);

    assert_eq!(
        dropped.get(),
        Some(7),
        "the drop lands where it was released"
    );
}
