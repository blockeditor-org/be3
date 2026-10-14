use super::*;
use crate::ForwardedInput;
use crate::KeyAt;
use crate::reactive::{Action, Canvas, CanvasItem, Chord, Frame, Interactive, build, view};

const KEY_F: u32 = 33;
const KEY_LEFTMETA: u32 = 125;

fn stroke(key: Key, code: u32, pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        press: KeyPress {
            key,
            pressed,
            repeat,
            modifiers,
        },
        code: Some(code),
        text: None,
    }
}

fn f(pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    stroke(Key::F, KEY_F, pressed, repeat, modifiers)
}

fn logo(pressed: bool) -> Event {
    let held = match pressed {
        true => Modifiers::LOGO,
        false => Modifiers::NONE,
    };
    stroke(Key::Logo, KEY_LEFTMETA, pressed, false, held)
}

#[test]
fn an_intercepted_press_keeps_its_physical_key_repeats_and_release_from_the_app() {
    let _root = crate::reactive::RootScope::new();
    let (ran, set_ran) = create_signal(0);
    let (forwarded, set_forwarded) = create_signal(Vec::<Event>::new());
    let document = build({
        move || {
            Action::new("test.fullscreen", "Fullscreen", move || {
                set_ran.update(|ran| *ran += 1)
            })
            .shortcut(Chord::logo(Key::F))
            .at(KeyAt::Window)
            .capture()
            .register();
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            on_forward={move |input: ForwardedInput| {
                                set_forwarded.update(|forwarded| forwarded.extend(flattened(input)));
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
    set_forwarded.update(|forwarded| forwarded.clear());

    harness.frame(vec![
        logo(true),
        Event::Modifiers(Modifiers::LOGO),
        f(true, false, Modifiers::LOGO),
    ]);
    harness.frame(vec![f(true, true, Modifiers::LOGO)]);
    assert_eq!(
        ran.get_untracked(),
        2,
        "the press and its repeat run the action"
    );
    harness.click(pos2(250.0, 50.0));
    harness.frame(vec![f(false, false, Modifiers::LOGO)]);
    harness.click(pos2(50.0, 50.0));
    harness.frame(vec![logo(false), Event::Modifiers(Modifiers::NONE)]);
    let keys: Vec<Event> = forwarded
        .get_untracked()
        .iter()
        .filter(|event| matches!(event, Event::Key { .. }))
        .cloned()
        .collect();
    assert_eq!(
        keys,
        [logo(true), logo(false)],
        "the app hears Super, but no part of the F that ran the action"
    );

    set_forwarded.update(|forwarded| forwarded.clear());
    harness.frame(vec![f(true, false, Modifiers::NONE)]);
    harness.frame(vec![f(false, false, Modifiers::NONE)]);
    assert!(
        forwarded
            .get_untracked()
            .iter()
            .filter(|event| matches!(event, Event::Key { code: Some(_), .. }))
            .count()
            == 2,
        "a plain F after the action reaches the app whole: {:?}",
        forwarded.get_untracked()
    );
}
