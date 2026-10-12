use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};
use crate::{Forwarded, ForwardedInput};

fn lefts(heard: &RefCell<Vec<Forwarded>>) -> usize {
    heard
        .borrow()
        .iter()
        .filter(|event| **event == Forwarded::Left)
        .count()
}

#[test]
fn a_forwarding_catcher_hears_the_pointer_leave_once_no_press_holds_it() {
    let heard = Rc::new(RefCell::new(Vec::<Forwarded>::new()));
    let document = build({
        let heard = Rc::clone(&heard);
        move || {
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            on_forward={move |input: ForwardedInput| {
                                heard.borrow_mut().extend(input.events)
                            }}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (inside, outside) = (pos2(50.0, 50.0), pos2(250.0, 50.0));

    harness.frame(vec![Event::PointerMoved(inside)]);
    harness.frame(vec![Event::PointerMoved(outside)]);
    harness.frame(vec![Event::PointerMoved(pos2(260.0, 50.0))]);
    assert_eq!(
        lefts(&heard),
        1,
        "the catcher hears the pointer leave, once"
    );

    heard.borrow_mut().clear();
    harness.press_at(inside);
    harness.frame(vec![Event::PointerMoved(outside)]);
    assert_eq!(lefts(&heard), 0, "a press it holds keeps the pointer");
    harness.frame(vec![Event::PointerButton {
        pos: outside,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(lefts(&heard), 1, "until it is let go");
}
