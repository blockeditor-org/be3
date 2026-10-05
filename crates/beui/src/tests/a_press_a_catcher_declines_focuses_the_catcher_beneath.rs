use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

#[test]
fn a_press_a_catcher_declines_focuses_the_catcher_beneath() {
    let below = Rc::new(RefCell::new(Vec::<Event>::new()));
    let above = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let (below, above) = (Rc::clone(&below), Rc::clone(&above));
        move || {
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                        <Interactive
                            focusable=true
                            on_forward={move |input: ForwardedInput| {
                                if input.focused {
                                    below.borrow_mut().extend(input.events);
                                }
                            }}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                        <Interactive
                            focusable=true
                            forward_at={|local: Pos2| local.x >= 100.0}
                            on_forward={move |input: ForwardedInput| {
                                if input.focused {
                                    above.borrow_mut().extend(input.events);
                                }
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

    harness.click(pos2(50.0, 50.0));
    below.borrow_mut().clear();
    above.borrow_mut().clear();
    harness.frame(vec![Event::Text("a".to_owned())]);
    assert_eq!(*below.borrow(), [Event::Text("a".to_owned())]);
    assert!(above.borrow().is_empty(), "the catcher that declined the press is not focused");

    harness.click(pos2(150.0, 50.0));
    below.borrow_mut().clear();
    above.borrow_mut().clear();
    harness.frame(vec![Event::Text("b".to_owned())]);
    assert_eq!(*above.borrow(), [Event::Text("b".to_owned())]);
    assert!(below.borrow().is_empty());
}
