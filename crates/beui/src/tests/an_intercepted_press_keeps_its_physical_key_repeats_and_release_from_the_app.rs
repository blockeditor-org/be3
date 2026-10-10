use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Action, Canvas, CanvasItem, Chord, Frame, Interactive, build, view};

const KEY_F: u32 = 33;
const KEY_LEFTMETA: u32 = 125;

fn f(pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key: Key::F,
        pressed,
        repeat,
        modifiers,
    }
}

fn physical(code: u32, pressed: bool) -> Event {
    Event::PhysicalKey { code, pressed }
}

#[test]
fn an_intercepted_press_keeps_its_physical_key_repeats_and_release_from_the_app() {
    let ran = Rc::new(Cell::new(0));
    let forwarded = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let (ran, forwarded) = (Rc::clone(&ran), Rc::clone(&forwarded));
        move || {
            Action::new("test.fullscreen", "Fullscreen", move || {
                ran.set(ran.get() + 1)
            })
            .shortcut(Chord::logo(Key::F))
            .intercepts()
            .register();
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            on_forward={move |input: ForwardedInput| {
                                forwarded.borrow_mut().extend(input.events);
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
    forwarded.borrow_mut().clear();

    harness.frame(vec![
        physical(KEY_LEFTMETA, true),
        Event::Modifiers(Modifiers::LOGO),
        physical(KEY_F, true),
        f(true, false, Modifiers::LOGO),
    ]);
    harness.frame(vec![f(true, true, Modifiers::LOGO)]);
    assert_eq!(ran.get(), 2, "the press and its repeat run the action");
    harness.click(pos2(250.0, 50.0));
    harness.frame(vec![
        physical(KEY_F, false),
        f(false, false, Modifiers::LOGO),
    ]);
    harness.click(pos2(50.0, 50.0));
    harness.frame(vec![
        physical(KEY_LEFTMETA, false),
        Event::Modifiers(Modifiers::NONE),
    ]);
    let keys: Vec<Event> = forwarded
        .borrow()
        .iter()
        .filter(|event| matches!(event, Event::PhysicalKey { .. } | Event::Key { .. }))
        .cloned()
        .collect();
    assert_eq!(
        keys,
        [physical(KEY_LEFTMETA, true), physical(KEY_LEFTMETA, false)],
        "the app hears Super, but no part of the F that ran the action"
    );

    forwarded.borrow_mut().clear();
    harness.frame(vec![physical(KEY_F, true), f(true, false, Modifiers::NONE)]);
    harness.frame(vec![
        physical(KEY_F, false),
        f(false, false, Modifiers::NONE),
    ]);
    assert!(
        forwarded
            .borrow()
            .iter()
            .filter(|event| matches!(event, Event::PhysicalKey { .. }))
            .count()
            == 2,
        "a plain F after the action reaches the app whole: {:?}",
        forwarded.borrow()
    );
}
