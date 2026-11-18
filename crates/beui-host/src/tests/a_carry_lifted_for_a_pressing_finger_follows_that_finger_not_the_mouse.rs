use std::cell::OnceCell;
use std::rc::Rc;

use super::*;
use crate::drag::{self, Carry, DragPoint};
use crate::PointerId;
use crate::unstyled::{drag_board, use_drop_site};

/// What a host does for a drag an app begins (`Editor::drag`): it lifts a carry from outside
/// any `Interactive`, at the pointer pressing (`Document::last_pointer`).
#[test]
fn a_carry_lifted_for_a_pressing_finger_follows_that_finger_not_the_mouse() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<Pos2>);
    let over = create_signal(Vec::<Pos2>::new());
    let board = Rc::new(OnceCell::new());
    let kept = Rc::clone(&board);
    let (document, [pad, basket]) = toolbar_of(move || {
        kept.get_or_init(drag_board);
        let basket = NodeRef::new();
        let site = use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| drag::custom(basket, |_: Pos2, point: DragPoint| Some(point.pos))),
            Callback::new(move |(_, at): (u32, Pos2)| dropped.set(Some(at))),
        );
        create_effect(move || {
            if let Some(at) = site.over.get() {
                over.update(|over| over.push(at));
            }
        });
        [
            view! {
                <Interactive touch_drags=true>
                    <Frame width=80.0 height=80.0 />
                </Interactive>
            },
            view! {
                <Frame @node_ref=&basket width=200.0 height=80.0 />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (pad, basket) = (harness.rect(pad), harness.rect(basket));

    harness.touch(TouchPhase::Start, pad.center());
    // The mouse moves after the finger pressed: its event is the latest.
    harness.frame(vec![Input::PointerMoved(basket.left_top() + vec2(10.0, 10.0))]);
    let pressing = harness
        .document()
        .last_pointer
        .expect("the document saw a pointer");
    assert!(
        matches!(pressing.pointer, Some(PointerId::Touch(_))),
        "the sample is the press, not the mouse's later move: {pressing:?}"
    );
    assert_eq!(pressing.pos, pad.center(), "at the finger's place");
    let board = board.get().expect("the board was made with the view");
    let point = DragPoint::at(pressing.pos, Modifiers::NONE)
        .in_window(pressing.window)
        .by(pressing.pointer.expect("a pointer presses"));
    board.lift(Carry::new(7_u32).drawn_by_site(), point);

    harness.frame(vec![Input::PointerMoved(basket.center())]);
    assert!(
        over.with_untracked(|over| !over.contains(&basket.center())),
        "the mouse does not move the finger's carry: {:?}",
        over.get_untracked()
    );
    let at = basket.center() + vec2(30.0, 0.0);
    harness.touch(TouchPhase::Move, at);
    harness.touch(TouchPhase::End, at);
    assert_eq!(
        dropped.get_untracked(),
        Some(at),
        "the finger carries it and drops it where it lets go"
    );
    assert!(!board.carrying());

    harness.frame(vec![Input::PointerMoved(pad.center())]);
    assert_eq!(
        harness.document().last_pointer.and_then(|sample| sample.pointer),
        None,
        "with nothing pressing, the sample names no pointer, so nothing is lifted for it"
    );
}
