use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

type Heard = Rc<RefCell<Vec<(usize, bool, Vec<Event>)>>>;

#[test]
fn f6_moves_the_focus_between_forwarding_catchers() {
    let heard: Heard = Rc::new(RefCell::new(Vec::new()));
    let catcher = |index: usize, heard: Heard| {
        move |input: ForwardedInput| {
            heard
                .borrow_mut()
                .push((index, input.focused, input.events))
        }
    };
    let document = build({
        let heard = Rc::clone(&heard);
        move || {
            let first = catcher(0, Rc::clone(&heard));
            let last = catcher(1, Rc::clone(&heard));
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive focusable=true on_forward={first}>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                        <Interactive focusable=true>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=400.0 y=0.0 width=100.0 height=100.0>
                        <Interactive focusable=true on_forward={last}>
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
    let f6 = |shift: bool| Event::Key {
        key: Key::F6,
        pressed: true,
        repeat: false,
        modifiers: match shift {
            true => Modifiers::SHIFT,
            false => Modifiers::NONE,
        },
    };

    heard.borrow_mut().clear();
    harness.frame(vec![f6(false)]);
    let focused = |heard: &[(usize, bool, Vec<Event>)]| {
        heard
            .iter()
            .rev()
            .find(|(_, focused, _)| *focused)
            .map(|(index, _, _)| *index)
    };
    assert_eq!(
        focused(&heard.borrow()),
        Some(1),
        "F6 skips the plain focusable"
    );
    assert!(
        heard
            .borrow()
            .iter()
            .all(|(_, _, events)| events.is_empty()),
        "the F6 itself reaches no catcher"
    );

    heard.borrow_mut().clear();
    harness.frame(vec![f6(true)]);
    assert_eq!(focused(&heard.borrow()), Some(0), "Shift+F6 goes back");
}
