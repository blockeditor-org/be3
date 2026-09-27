use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{ClickCatcher, Frame, List, build, view};

#[test]
fn a_touch_beside_a_control_reaches_the_nearest_one() {
    let clicks: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let document = build({
        let clicks = clicks.clone();
        move || {
            let (left, right) = (clicks.clone(), clicks.clone());
            view! {
                <Frame padding_horizontal=40.0 padding_vertical=40.0>
                    <List direction=Direction::Horizontal spacing=10.0>
                        <ClickCatcher on_click={move || left.borrow_mut().push("left")}>
                            <Frame width=30.0 height=30.0 />
                        </ClickCatcher>
                        <ClickCatcher on_click={move || right.borrow_mut().push("right")}>
                            <Frame width=30.0 height=30.0 />
                        </ClickCatcher>
                    </List>
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let tap = |harness: &mut Harness, pos: Pos2| {
        harness.touch(TouchPhase::Start, pos);
        harness.touch(TouchPhase::End, pos);
    };

    tap(&mut harness, pos2(32.0, 55.0));
    tap(&mut harness, pos2(73.0, 55.0));
    tap(&mut harness, pos2(77.0, 55.0));
    tap(&mut harness, pos2(95.0, 82.0));
    assert_eq!(
        *clicks.borrow(),
        vec!["left", "left", "right", "right"],
        "a touch beside a control goes to the nearest one, and neighbours split the gap"
    );

    clicks.borrow_mut().clear();
    tap(&mut harness, pos2(10.0, 55.0));
    harness.click(pos2(32.0, 55.0));
    assert!(
        clicks.borrow().is_empty(),
        "a touch out of reach, and a mouse beside a control, press nothing"
    );
}
