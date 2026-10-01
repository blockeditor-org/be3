use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

#[test]
fn a_pressed_forwarding_catcher_keeps_the_pointer_until_it_is_released() {
    let log = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let log = Rc::clone(&log);
        move || {
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            on_forward={move |input: ForwardedInput| {
                                log.borrow_mut().extend(input.events)
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

    harness.press_at(pos2(50.0, 50.0));
    harness.frame(vec![Event::PointerMoved(pos2(300.0, 250.0))]);
    harness.release_at(pos2(300.0, 250.0));
    harness.frame(vec![Event::PointerMoved(pos2(310.0, 250.0))]);

    let log = log.borrow();
    assert!(log.contains(&Event::PointerMoved(pos2(300.0, 250.0))));
    assert!(log.iter().any(|event| matches!(
        event,
        Event::PointerButton { pressed: false, pos, .. } if *pos == pos2(300.0, 250.0)
    )));
    assert!(
        !log.contains(&Event::PointerMoved(pos2(310.0, 250.0))),
        "once released, a move outside is not the catcher's"
    );
}
