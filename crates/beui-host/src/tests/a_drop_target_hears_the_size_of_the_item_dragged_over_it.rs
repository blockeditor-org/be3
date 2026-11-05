use std::cell::OnceCell;
use std::rc::Rc;

use super::*;
use crate::unstyled::{
    Board, DragHandle, DragPoint, Draggable, DropHandle, DropTarget, drag_board,
};

#[test]
fn a_drop_target_hears_the_size_of_the_item_dragged_over_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(None::<Rect>);
    let board = Rc::new(OnceCell::<Rc<Board>>::new());
    let kept = Rc::clone(&board);
    let (document, [source, target]) = toolbar_of(move || {
        kept.get_or_init(drag_board);
        [
            view! {
                <Draggable
                    payload={7_u32}
                    preview={|_: u32| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                >|_: DragHandle|
                    <Frame width=40.0 height=30.0 />
                </Draggable>
            },
            view! {
                <DropTarget
                    on_over={move |over: Option<(u32, DragPoint)>| {
                        if let Some((_, point)) = over {
                            heard.set(Some(point.item));
                        }
                    }}
                >|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(source).height(), 30.0);

    let grabbed = harness.rect(source).min + Vec2::new(10.0, 5.0);
    harness.down(grabbed, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(harness.center(target))]);

    assert_eq!(
        heard.get_untracked(),
        Some(Rect::from_min_size(
            pos2(-10.0, -5.0),
            harness.rect(source).size()
        )),
        "the target hears the carried item's rect, relative to where it was grabbed"
    );
    assert_eq!(
        board
            .get()
            .expect("the board was made with the view")
            .item(),
        heard.get_untracked(),
        "and the board answers the same"
    );
}
