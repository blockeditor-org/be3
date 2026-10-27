use std::rc::Rc;

use super::*;
use crate::unstyled::{DragPoint, DragSource, DropHandle, DropTarget};

#[test]
fn a_drag_on_the_board_drops_nothing_when_its_touch_is_cancelled() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<u32>);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Frame width=40.0 height=40.0 />
            },
            view! {
                <DropTarget
                    on_drop={move |(payload, _): (u32, DragPoint)| dropped.set(Some(payload))}
                >|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let from = harness.center(source);
    let to = harness.center(target);
    harness.touch(TouchPhase::Start, from);
    harness.document().drag_board().begin(
        DragSource::new(Rc::new(7_u32)),
        DragPoint::at(from, Modifiers::NONE),
    );
    harness.touch(TouchPhase::Move, to);
    harness.touch(TouchPhase::Cancel, to);

    assert_eq!(
        dropped.get_untracked(),
        None,
        "a cancelled touch drops nothing"
    );
    assert!(
        !harness.document().dragging(),
        "but the board lets go of what it carried"
    );
}
