use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::KeyAt;
use crate::ForwardedInput;
use crate::reactive::{
    Action, Canvas, CanvasItem, Chord, Frame, Interactive, WriteSignal, build, create_signal, view,
    with_reactive_scope,
};

#[test]
fn an_app_that_inhibits_shortcuts_gets_every_chord_but_the_escape_hatch() {
    let forwarded = Rc::new(RefCell::new(Vec::<Key>::new()));
    let ran = Rc::new(RefCell::new(Vec::new()));
    let setter: Rc<RefCell<Option<WriteSignal<bool>>>> = Rc::default();
    let document = build({
        let forwarded = Rc::clone(&forwarded);
        let (intercepting, hatch) = (Rc::clone(&ran), Rc::clone(&ran));
        let setter = Rc::clone(&setter);
        move || {
            Action::new("test.intercepting", "Intercepting", move || {
                intercepting.borrow_mut().push("intercepting")
            })
            .shortcut(Chord::logo(Key::H))
            .at(KeyAt::Window).capture()
            .register();
            Action::new("test.hatch", "Escape hatch", move || {
                hatch.borrow_mut().push("hatch")
            })
            .shortcut(Chord::logo(Key::L))
            .at(KeyAt::Host).capture()
            .register();
            let (inhibits, set_inhibits) = create_signal(true);
            *setter.borrow_mut() = Some(set_inhibits);
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            inhibits_shortcuts={inhibits}
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
    let set_inhibits = setter.borrow().clone().expect("the document was built");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));

    harness.key(Key::H, Modifiers::LOGO);
    harness.key(Key::F6, Modifiers::NONE);
    harness.key(Key::L, Modifiers::LOGO);
    assert_eq!(
        *forwarded.borrow(),
        [Key::H, Key::F6],
        "an app that inhibits shortcuts gets intercepted chords and F6, but not the escape hatch"
    );
    assert_eq!(*ran.borrow(), ["hatch"]);

    with_reactive_scope(&mut harness.document, || set_inhibits.set(false));
    harness.key(Key::H, Modifiers::LOGO);
    assert_eq!(
        *ran.borrow(),
        ["hatch", "intercepting"],
        "once it lets go, the intercepting action answers again"
    );

    with_reactive_scope(&mut harness.document, || set_inhibits.set(true));
    harness.click(pos2(250.0, 50.0));
    harness.key(Key::H, Modifiers::LOGO);
    assert_eq!(
        *ran.borrow(),
        ["hatch", "intercepting", "intercepting"],
        "an app that inhibits shortcuts but does not have the focus inhibits nothing"
    );
}
