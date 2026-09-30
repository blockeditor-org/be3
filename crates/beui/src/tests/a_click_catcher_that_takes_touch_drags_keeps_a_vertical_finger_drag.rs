use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::input::PointerPress;
use crate::reactive::{ClickCatcher, Frame, build, view};

#[test]
fn a_click_catcher_that_takes_touch_drags_keeps_a_vertical_finger_drag() {
    let drags: Rc<RefCell<Vec<Pos2>>> = Rc::default();
    let cancelled = Rc::new(Cell::new(false));
    let document = build({
        let (drags, cancelled) = (drags.clone(), cancelled.clone());
        move || {
            view! {
                <ClickCatcher
                    touch_drags=true
                    on_drag={move |press: PointerPress| drags.borrow_mut().push(press.pos)}
                    on_cancel={move || cancelled.set(true)}
                >
                    <Frame width=300.0 height=300.0 />
                </ClickCatcher>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.touch(TouchPhase::Start, pos2(100.0, 50.0));
    harness.touch(TouchPhase::Move, pos2(100.0, 120.0));
    harness.touch(TouchPhase::Move, pos2(102.0, 200.0));
    harness.touch(TouchPhase::End, pos2(102.0, 200.0));

    assert!(
        !cancelled.get(),
        "a vertical drag is not taken for a scroll"
    );
    assert_eq!(drags.borrow().last(), Some(&pos2(102.0, 200.0)));
}
