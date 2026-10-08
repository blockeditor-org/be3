use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::input::SecondaryDrag;
use crate::reactive::{Frame, Interactive, build, view};

#[test]
fn a_secondary_drag_released_as_the_pointer_leaves_still_ends() {
    let drags = Rc::new(RefCell::new(Vec::<SecondaryDrag>::new()));
    let document = build({
        let drags = Rc::clone(&drags);
        move || {
            view! {
                <Interactive
                    on_secondary_drag={move |drag: SecondaryDrag| drags.borrow_mut().push(drag)}
                >
                    <Frame />
                </Interactive>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (from, to) = (pos2(50.0, 50.0), pos2(120.0, 90.0));
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(to)]);
    harness.frame(vec![
        Event::PointerButton {
            pos: to,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
        Event::PointerGone,
    ]);

    let last = *drags.borrow().last().expect("the drag was heard");
    assert!(last.ended, "the release ended the drag");
    assert!(
        !last.cancelled,
        "the pointer leaving after the release does not take the drag back"
    );
    assert_eq!(last.pos, to);
}
