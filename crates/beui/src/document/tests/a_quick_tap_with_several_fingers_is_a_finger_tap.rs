use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Frame, build, on_finger_tap, view};

#[test]
fn a_quick_tap_with_several_fingers_is_a_finger_tap() {
    let taps: Rc<RefCell<Vec<usize>>> = Rc::default();
    let document = build({
        let taps = taps.clone();
        move || {
            on_finger_tap(move |fingers: usize| {
                taps.borrow_mut().push(fingers);
                true
            });
            view! {
                <Frame width=300.0 height=300.0 />
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.touch(TouchPhase::Start, pos2(50.0, 50.0));
    harness.touch(TouchPhase::End, pos2(50.0, 50.0));
    assert!(taps.borrow().is_empty(), "one finger is an ordinary tap");

    harness.finger(1, TouchPhase::Start, pos2(50.0, 50.0));
    harness.finger(2, TouchPhase::Start, pos2(120.0, 50.0));
    harness.finger(3, TouchPhase::Start, pos2(190.0, 50.0));
    harness.finger(3, TouchPhase::End, pos2(190.0, 50.0));
    harness.finger(2, TouchPhase::End, pos2(120.0, 50.0));
    harness.finger(1, TouchPhase::End, pos2(50.0, 50.0));
    assert_eq!(
        *taps.borrow(),
        vec![3],
        "the tap counts the most fingers that were down"
    );

    harness.finger(1, TouchPhase::Start, pos2(50.0, 50.0));
    harness.finger(2, TouchPhase::Start, pos2(120.0, 50.0));
    harness.move_fingers(pos2(30.0, 50.0), pos2(160.0, 50.0));
    harness.finger(1, TouchPhase::End, pos2(30.0, 50.0));
    harness.finger(2, TouchPhase::End, pos2(160.0, 50.0));
    assert_eq!(
        *taps.borrow(),
        vec![3],
        "fingers that moved were a pinch, not a tap"
    );
}
