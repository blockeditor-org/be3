use std::rc::Rc;

use super::*;
use crate::unstyled::{DragPoint, DropHandle, DropTarget};

#[test]
fn a_drag_put_on_the_board_drops_where_the_button_is_let_go() {
    let dropped = Rc::new(Cell::new(None::<(u32, Pos2)>));
    let received = Rc::clone(&dropped);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Frame width=40.0 height=40.0 />
            },
            view! {
                <DropTarget
                    on_drop={move |(payload, point): (u32, DragPoint)| {
                        received.set(Some((payload, point.pos)))
                    }}
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
    let to = harness.center(target);
    harness.press_at(from);
    harness.document().drag_board().begin(
        Rc::new(7_u32),
        DragPoint {
            pos: from,
            modifiers: Modifiers::NONE,
        },
        Rc::new(|_| {}),
    );
    harness.frame(vec![Event::PointerMoved(to)]);
    assert_eq!(
        dropped.get(),
        None,
        "nothing drops while the button is held"
    );

    harness.release_at(to);
    assert_eq!(
        dropped.get(),
        Some((7, to)),
        "a drag that no Draggable began still drops where the button is let go"
    );
    assert!(
        !harness.document().dragging(),
        "and the board carries nothing afterwards"
    );
}
