use std::cell::OnceCell;
use std::rc::Rc;

use super::*;
use crate::unstyled::{Board, DragPoint, DropHandle, DropTarget, drag_board};

#[test]
fn a_drag_on_the_board_drops_nothing_when_its_touch_is_cancelled() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<u32>);
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
    board
        .get()
        .expect("the board was made with the view")
        .begin(
            Rc::new(7_u32),
            DragPoint::at(from, Modifiers::NONE),
            Rc::new(|_| {}),
            Rc::new(|_| {}),
        );
    harness.touch(TouchPhase::Move, to);
    harness.touch(TouchPhase::Cancel, to);

    assert_eq!(
        dropped.get_untracked(),
        None,
        "a cancelled touch drops nothing"
    );
    assert!(
        !board
            .get()
            .expect("the board was made with the view")
            .carrying(),
        "but the board lets go of what it carried"
    );
}
