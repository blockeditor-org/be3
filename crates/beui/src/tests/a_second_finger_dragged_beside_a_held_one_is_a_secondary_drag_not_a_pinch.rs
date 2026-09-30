use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::input::{SecondaryDrag, ZoomGesture};
use crate::reactive::{ClickCatcher, Frame, build, view};

#[test]
fn a_second_finger_dragged_beside_a_held_one_is_a_secondary_drag_not_a_pinch() {
    let drags: Rc<RefCell<Vec<SecondaryDrag>>> = Rc::default();
    let zoomed = Rc::new(Cell::new(false));
    let document = build({
        let (drags, zoomed) = (drags.clone(), zoomed.clone());
        move || {
            view! {
                <ClickCatcher
                    on_secondary_drag={move |drag| drags.borrow_mut().push(drag)}
                    on_zoom={move |_: ZoomGesture| zoomed.set(true)}
                >
                    <Frame width=300.0 height=300.0 />
                </ClickCatcher>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.finger(1, TouchPhase::Start, pos2(20.0, 20.0));
    harness.finger(2, TouchPhase::Start, pos2(100.0, 100.0));
    harness.finger(2, TouchPhase::Move, pos2(200.0, 150.0));
    harness.finger(2, TouchPhase::End, pos2(250.0, 160.0));
    harness.finger(1, TouchPhase::End, pos2(20.0, 20.0));
    harness.frame(Vec::new());

    assert!(!zoomed.get(), "a held finger does not pinch");
    let seen = drags.borrow();
    let last = seen.last().expect("the drag was reported");
    assert!(seen[0].started);
    assert_eq!(last.from, pos2(100.0, 100.0));
    assert_eq!(last.pos, pos2(250.0, 160.0));
    assert!(last.ended && !last.cancelled);

    drop(seen);
    harness.finger(1, TouchPhase::Start, pos2(20.0, 20.0));
    harness.finger(2, TouchPhase::Start, pos2(100.0, 100.0));
    harness.move_fingers(pos2(0.0, 0.0), pos2(150.0, 150.0));
    harness.frame(Vec::new());

    assert!(zoomed.get(), "moving the first finger too makes it a pinch");
    let cancelled = drags.borrow().last().is_some_and(|drag| drag.cancelled);
    assert!(cancelled, "the drag the second finger began is called off");
}
