use std::cell::OnceCell;
use std::rc::Rc;

use super::*;
use crate::unstyled::{Board, DragPoint, DropHandle, DropTarget, drag_board};

#[test]
fn a_drag_put_on_the_board_drops_where_the_button_is_let_go() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<(u32, Pos2)>);
    let board = Rc::new(OnceCell::<Rc<Board>>::new());
    let kept = Rc::clone(&board);
    let (document, [source, target]) = toolbar_of(move || {
        kept.get_or_init(drag_board);
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
    harness.down(from, PointerButton::Primary);
    board
        .get()
        .expect("the board was made with the view")
        .begin(
            Rc::new(7_u32),
            DragPoint::at(from, Modifiers::NONE),
            Rc::new(|_| {}),
            Rc::new(|_| {}),
        );
    harness.frame(vec![Input::PointerMoved(to)]);
    assert_eq!(
        dropped.get_untracked(),
        None,
        "nothing drops while the button is held"
    );

    harness.up(to, PointerButton::Primary);
    assert_eq!(
        dropped.get_untracked(),
        Some((7, to)),
        "a drag that no Draggable began still drops where the button is let go"
    );
    assert!(
        !board
            .get()
            .expect("the board was made with the view")
            .carrying(),
        "and the board carries nothing afterwards"
    );
}
