use std::rc::Rc;

use super::*;
use crate::unstyled::{DragPoint, DragSource, DropHandle, DropTarget};

#[test]
fn a_drag_put_on_the_board_drops_where_the_button_is_let_go() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<(u32, Pos2)>);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Frame width=40.0 height=40.0 />
            },
            view! {
                <DropTarget
                    on_drop={move |(payload, point): (u32, DragPoint)| {
                        dropped.set(Some((payload, point.pos)))
                    }}
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
    harness.press_at(from);
    harness.document().drag_board().begin(
        DragSource::new(Rc::new(7_u32)),
        DragPoint::at(from, Modifiers::NONE),
    );
    harness.frame(vec![Input::PointerMoved(to)]);
    assert_eq!(
        dropped.get_untracked(),
        None,
        "nothing drops while the button is held"
    );

    harness.release_at(to);
    assert_eq!(
        dropped.get_untracked(),
        Some((7, to)),
        "a drag that no Draggable began still drops where the button is let go"
    );
    assert!(
        !harness.document().dragging(),
        "and the board carries nothing afterwards"
    );
}
