use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

type Heard = Rc<RefCell<Vec<(usize, Event)>>>;

#[test]
fn a_catcher_that_lost_its_release_lets_go_of_the_pointer() {
    let heard: Heard = Rc::new(RefCell::new(Vec::new()));
    let catcher = |index: usize, heard: Heard| {
        move |input: ForwardedInput| {
            heard
                .borrow_mut()
                .extend(input.events.into_iter().map(|event| (index, event)))
        }
    };
    let document = build({
        let heard = Rc::clone(&heard);
        move || {
            let first = catcher(0, Rc::clone(&heard));
            let second = catcher(1, Rc::clone(&heard));
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive on_forward={first}>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                        <Interactive on_forward={second}>
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
    harness.frame(vec![Event::Focus(false)]);
    heard.borrow_mut().clear();
    harness.frame(vec![Event::PointerMoved(pos2(250.0, 50.0))]);

    assert_eq!(
        *heard.borrow(),
        [(1, Event::PointerMoved(pos2(250.0, 50.0)))],
        "with no button down any more, the move goes to the catcher under the pointer"
    );
}
