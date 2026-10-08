use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Action, Canvas, CanvasItem, Chord, Frame, Interactive, build, view};

#[test]
fn a_focused_app_keeps_its_keys_from_global_actions_that_do_not_intercept() {
    let forwarded = Rc::new(RefCell::new(Vec::<Key>::new()));
    let ran = Rc::new(RefCell::new(Vec::new()));
    let document = build({
        let forwarded = Rc::clone(&forwarded);
        let (global, intercepting) = (Rc::clone(&ran), Rc::clone(&ran));
        move || {
            Action::new("test.global", "Global", move || {
                global.borrow_mut().push("global")
            })
            .shortcut(Chord::logo(Key::G))
            .global()
            .register();
            Action::new("test.intercepting", "Intercepting", move || {
                intercepting.borrow_mut().push("intercepting")
            })
            .shortcut(Chord::logo(Key::H))
            .intercepts()
            .register();
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            on_forward={move |input: ForwardedInput| {
                                if input.focused {
                                    forwarded.borrow_mut().extend(input.events.iter().filter_map(
                                        |event| match event {
                                            Event::Key { key, pressed: true, .. } => Some(*key),
                                            _ => None,
                                        },
                                    ));
                                }
                            }}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                        <Interactive focusable=true>
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

    harness.key(Key::G, Modifiers::LOGO);
    harness.key(Key::H, Modifiers::LOGO);
    assert_eq!(
        *forwarded.borrow(),
        [Key::G],
        "the app gets the chord of a global action, but not of one that intercepts"
    );
    assert_eq!(*ran.borrow(), ["intercepting"]);

    harness.click(pos2(250.0, 50.0));
    harness.key(Key::G, Modifiers::LOGO);
    assert_eq!(
        *ran.borrow(),
        ["intercepting", "global"],
        "with beui's own control focused, the global action answers"
    );
}
