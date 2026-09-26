use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::input::SecondaryDrag;
use crate::reactive::{ClickCatcher, Frame, build, view};

#[test]
fn right_dragging_over_a_click_catcher_reports_a_secondary_drag() {
    let drags: Rc<RefCell<Vec<SecondaryDrag>>> = Rc::default();
    let document = build({
        let drags = drags.clone();
        move || {
            view! {
                <ClickCatcher on_secondary_drag={move |drag| drags.borrow_mut().push(drag)}>
                    <Frame width=200.0 height=200.0 />
                </ClickCatcher>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::SHIFT,
    };

    harness.frame(vec![Event::PointerMoved(pos2(20.0, 30.0))]);
    harness.frame(vec![button(pos2(20.0, 30.0), true)]);
    harness.frame(vec![Event::PointerMoved(pos2(120.0, 90.0))]);
    harness.frame(vec![button(pos2(150.0, 100.0), false)]);
    harness.frame(Vec::new());

    let drags = drags.borrow();
    let seen: Vec<(Pos2, Pos2, bool, bool)> = drags
        .iter()
        .map(|drag| (drag.from, drag.pos, drag.started, drag.ended))
        .collect();
    assert_eq!(
        seen,
        [
            (pos2(20.0, 30.0), pos2(20.0, 30.0), true, false),
            (pos2(20.0, 30.0), pos2(120.0, 90.0), false, false),
            (pos2(20.0, 30.0), pos2(150.0, 100.0), false, true),
        ]
    );
    assert!(drags.iter().all(|drag| drag.modifiers == Modifiers::SHIFT));
}
