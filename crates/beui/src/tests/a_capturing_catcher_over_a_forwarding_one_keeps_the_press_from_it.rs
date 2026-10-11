use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::input::PointerPress;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

fn presses(forwarded: &RefCell<Vec<ForwardedInput>>) -> usize {
    forwarded
        .borrow()
        .iter()
        .flat_map(|input| input.events.iter())
        .filter(|event| matches!(event, Event::PointerButton { pressed: true, .. }))
        .count()
}

#[test]
fn a_capturing_catcher_over_a_forwarding_one_keeps_the_press_from_it() {
    let forwarded = Rc::new(RefCell::new(Vec::<ForwardedInput>::new()));
    let pressed = Rc::new(Cell::new(0));
    let document = build({
        let (forwarded, pressed) = (Rc::clone(&forwarded), Rc::clone(&pressed));
        move || {
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                        <Interactive on_forward={move |input| forwarded.borrow_mut().push(input)}>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=50.0 y=0.0 width=10.0 height=200.0>
                        <Interactive
                            capture_presses=true
                            on_press={move |_: PointerPress| pressed.set(pressed.get() + 1)}
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

    harness.click(pos2(55.0, 100.0));
    assert_eq!(pressed.get(), 1);
    assert_eq!(presses(&forwarded), 0, "the catcher beneath is covered");

    harness.click(pos2(150.0, 100.0));
    assert_eq!(presses(&forwarded), 1);
}
