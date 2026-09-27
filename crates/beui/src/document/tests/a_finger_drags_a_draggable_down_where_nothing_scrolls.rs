use super::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn a_finger_drags_a_draggable_down_where_nothing_scrolls() {
    let dropped = Rc::new(Cell::new(None::<u32>));
    let received = Rc::clone(&dropped);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable payload={7_u32}>
                    {|_: DragHandle| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                </Draggable>
            },
            view! {
                <DropTarget
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
    harness.frame(Vec::new());
    let from = harness.center(source);
    let to = pos2(from.x, harness.center(target).y);
    assert!(to.y - from.y > 40.0, "the target is below the source");

    harness.finger_drag(&[from, pos2(from.x, from.y + 20.0), to]);

    assert_eq!(
        dropped.get(),
        Some(7),
        "a vertical finger drag with no scroll around it carries the payload"
    );
}
