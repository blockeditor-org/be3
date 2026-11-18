use std::cell::OnceCell;
use std::rc::Rc;

use super::*;
use crate::drag::{self, Carry};
use crate::unstyled::{drag_board, place_on, use_drop_site};

#[test]
fn a_carry_whose_touch_is_cancelled_drops_nothing() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<u32>);
    let board = Rc::new(OnceCell::new());
    let kept = Rc::clone(&board);
    let (document, [source, target]) = toolbar_of(move || {
        kept.get_or_init(drag_board);
        let basket = NodeRef::new();
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| place_on(basket, ()).map_or(drag::floating(), drag::fixed)),
            Callback::new(move |(payload, ()): (u32, ())| dropped.set(Some(payload))),
        );
        [
            view! {
                <Interactive
                    touch_drags=true
                    drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                >
                    <Frame width=40.0 height=40.0 />
                </Interactive>
            },
            view! {
                <Frame @node_ref=&basket width=100.0 height=100.0 />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let (from, to) = (harness.center(source), harness.center(target));
    harness.touch(TouchPhase::Start, from);
    harness.touch(TouchPhase::Move, to);
    assert!(
        board
            .get()
            .expect("the board was made with the view")
            .carrying(),
        "the finger lifted the carry past its threshold"
    );
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
