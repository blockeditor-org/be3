use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

fn moves(log: &RefCell<Vec<ForwardedInput>>) -> Vec<Pos2> {
    log.borrow()
        .iter()
        .flat_map(|input| input.events.iter())
        .filter_map(|event| match event {
            Event::PointerMoved(pos) => Some(*pos),
            _ => None,
        })
        .collect()
}

#[test]
fn forwarded_input_reaches_only_the_topmost_catcher_under_it() {
    let below = Rc::new(RefCell::new(Vec::<ForwardedInput>::new()));
    let above = Rc::new(RefCell::new(Vec::<ForwardedInput>::new()));
    let document = build({
        let (below, above) = (Rc::clone(&below), Rc::clone(&above));
        move || {
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                        <Interactive on_forward={move |input| below.borrow_mut().push(input)}>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=50.0 y=50.0 width=50.0 height=50.0>
                        <Interactive on_forward={move |input| above.borrow_mut().push(input)}>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![Event::PointerMoved(pos2(60.0, 60.0))]);
    assert_eq!(moves(&above), [pos2(60.0, 60.0)]);
    assert!(moves(&below).is_empty(), "the catcher beneath is covered");
    assert!(above.borrow().last().is_some_and(|input| input.hovered));

    harness.frame(vec![Event::PointerMoved(pos2(150.0, 150.0))]);
    assert_eq!(moves(&below), [pos2(150.0, 150.0)]);
    assert!(
        above.borrow().last().is_some_and(|input| !input.hovered),
        "the catcher the pointer left hears that it left"
    );
}
