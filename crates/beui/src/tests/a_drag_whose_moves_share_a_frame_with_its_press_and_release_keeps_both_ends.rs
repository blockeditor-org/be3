use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::input::PointerPress;
use crate::reactive::{Frame, Interactive, build, view};

#[test]
fn a_drag_whose_moves_share_a_frame_with_its_press_and_release_keeps_both_ends() {
    let presses: Rc<RefCell<Vec<Pos2>>> = Rc::default();
    let drags: Rc<RefCell<Vec<Pos2>>> = Rc::default();
    let document = build({
        let (presses, drags) = (presses.clone(), drags.clone());
        move || {
            view! {
                <Interactive
                    touch_drags=true
                    on_press={move |press: PointerPress| presses.borrow_mut().push(press.pos)}
                    on_drag={move |press: PointerPress| drags.borrow_mut().push(press.pos)}
                >
                    <Frame width=300.0 height=300.0 />
                </Interactive>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![
        touch_event(1, TouchPhase::Start, pos2(100.0, 100.0)),
        touch_event(1, TouchPhase::Move, pos2(103.0, 100.0)),
    ]);
    harness.frame(vec![
        touch_event(1, TouchPhase::Move, pos2(103.0, 103.0)),
        touch_event(1, TouchPhase::End, pos2(103.0, 103.0)),
    ]);

    assert_eq!(
        *presses.borrow(),
        vec![pos2(100.0, 100.0)],
        "the press is where the finger landed"
    );
    assert_eq!(
        *drags.borrow(),
        vec![pos2(103.0, 100.0), pos2(103.0, 103.0)],
        "the move made in the frame of the release is still dragged to"
    );
}
