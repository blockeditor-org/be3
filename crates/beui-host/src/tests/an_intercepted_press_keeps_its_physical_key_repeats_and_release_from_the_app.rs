use super::*;
use crate::ForwardedInput;
use crate::KeyAt;
use crate::primitives::*;
use crate::reactive::*;

const KEY_F: u32 = 33;
const KEY_LEFTMETA: u32 = 125;

fn stroke(key: Key, code: u32, pressed: bool, repeat: bool) -> Input {
    Input::Key(beui::Keystroke {
        key: Some(key),
        pressed,
        repeat,
        evdev: Some(code),
        text: None,
    })
}

fn f(pressed: bool, repeat: bool) -> Input {
    stroke(Key::F, KEY_F, pressed, repeat)
}

fn logo(pressed: bool) -> Vec<Input> {
    let held = match pressed {
        true => Modifiers::LOGO,
        false => Modifiers::NONE,
    };
    vec![
        Input::Modifiers(held),
        stroke(Key::Logo, KEY_LEFTMETA, pressed, false),
    ]
}

fn heard_logo(pressed: bool) -> Event {
    Event::Key {
        press: KeyPress {
            key: Key::Logo,
            pressed,
            repeat: false,
            modifiers: match pressed {
                true => Modifiers::LOGO,
                false => Modifiers::NONE,
            },
        },
        evdev: Some(KEY_LEFTMETA),
        text: None,
    }
}

#[test]
fn an_intercepted_press_keeps_its_physical_key_repeats_and_release_from_the_app() {
    let _root = crate::reactive::RootScope::new();
    let ran = create_signal(0);
    let forwarded = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        Action::new("test.fullscreen", "Fullscreen", move || {
            ran.update(|ran| *ran += 1)
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
                            forwarded.update(|forwarded| forwarded.extend(flattened(input)));
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
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    forwarded.update(|forwarded| forwarded.clear());

    harness.frame([logo(true), vec![f(true, false)]].concat());
    harness.frame(vec![f(true, true)]);
    assert_eq!(
        ran.get_untracked(),
        2,
        "the press and its repeat run the action"
    );
    harness.click(pos2(250.0, 50.0));
    harness.frame(vec![f(false, false)]);
    harness.click(pos2(50.0, 50.0));
    harness.frame(logo(false));
    let keys: Vec<Event> = forwarded
        .get_untracked()
        .iter()
        .filter(|event| matches!(event, Event::Key { .. }))
        .cloned()
        .collect();
    assert_eq!(
        keys,
        [heard_logo(true), heard_logo(false)],
        "the app hears Super, but no part of the F that ran the action"
    );

    forwarded.update(|forwarded| forwarded.clear());
    harness.frame(vec![f(true, false)]);
    harness.frame(vec![f(false, false)]);
    assert!(
        forwarded
            .get_untracked()
            .iter()
            .filter(|event| matches!(event, Event::Key { evdev: Some(_), .. }))
            .count()
            == 2,
        "a plain F after the action reaches the app whole: {:?}",
        forwarded.get_untracked()
    );
}
