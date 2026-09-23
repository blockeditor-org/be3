use super::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn dragging_onto_a_drop_target_hands_it_the_payload() {
    let dropped = Rc::new(Cell::new(None::<u32>));
    let clicks = Rc::new(Cell::new(0));
    let received = Rc::clone(&dropped);
    let clicked = Rc::clone(&clicks);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable payload={7_u32} on_click={move || clicked.set(clicked.get() + 1)}>
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

    harness.click(harness.center(source));
    assert_eq!(clicks.get(), 1, "a press that never moved is a click");
    assert_eq!(dropped.get(), None);

    harness.drag(harness.center(source), harness.center(target));
    assert_eq!(
        dropped.get(),
        Some(7),
        "the target is handed what was dragged"
    );
    assert_eq!(clicks.get(), 1, "a drag is not also a click");
}
