use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::{ClickCatcher, Frame, build, view};

#[test]
fn a_second_finger_cancels_the_press_the_first_began() {
    let cancelled = Rc::new(Cell::new(0));
    let clicked = Rc::new(Cell::new(false));
    let document = build({
        let (cancelled, clicked) = (cancelled.clone(), clicked.clone());
        move || {
            view! {
                <ClickCatcher
                    on_cancel={move || cancelled.set(cancelled.get() + 1)}
                    on_click={move || clicked.set(true)}
                >
                    <Frame width=300.0 height=300.0 />
                </ClickCatcher>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.finger(1, TouchPhase::Start, pos2(20.0, 20.0));
    harness.finger(1, TouchPhase::End, pos2(20.0, 20.0));
    assert_eq!(cancelled.get(), 0, "a tap is not cancelled");
    assert!(clicked.replace(false));

    harness.finger(1, TouchPhase::Start, pos2(100.0, 100.0));
    harness.finger(2, TouchPhase::Start, pos2(200.0, 200.0));
    harness.finger(2, TouchPhase::End, pos2(200.0, 200.0));
    harness.finger(1, TouchPhase::End, pos2(100.0, 100.0));

    assert_eq!(
        cancelled.get(),
        1,
        "the second finger cancels the press once"
    );
    assert!(!clicked.get(), "a cancelled press is not a click");
}
