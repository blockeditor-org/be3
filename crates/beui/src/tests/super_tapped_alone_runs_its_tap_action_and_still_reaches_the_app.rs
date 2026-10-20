use super::*;
use crate::ForwardedInput;
use crate::KeyAt;
use crate::primitives::*;
use crate::reactive::*;

fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::key(beui::KeyPress {
        key,
        pressed,
        repeat: false,
        modifiers,
    })
}

#[test]
fn super_tapped_alone_runs_its_tap_action_and_still_reaches_the_app() {
    let _root = crate::reactive::RootScope::new();
    let taps = create_signal(0);
    let forwarded = create_signal(Vec::<Event>::new());
    let document = build({
        move || {
            Action::new("test.tap", "Tap", move || taps.update(|taps| *taps += 1))
                .shortcut(Chord::tap(Key::Logo))
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
                </Canvas>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    forwarded.update(|forwarded| forwarded.clear());

    harness.frame(vec![
        key(Key::Logo, true, Modifiers::LOGO),
        Event::Modifiers(Modifiers::LOGO),
    ]);
    harness.frame(vec![
        key(Key::Logo, false, Modifiers::NONE),
        Event::Modifiers(Modifiers::NONE),
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super pressed and let go alone is a tap"
    );
    assert!(
        forwarded
            .get_untracked()
            .contains(&key(Key::Logo, false, Modifiers::NONE)),
        "the app focused still gets the Super it saw pressed let go"
    );

    harness.frame(vec![
        key(Key::Logo, true, Modifiers::LOGO),
        key(Key::T, true, Modifiers::LOGO),
        key(Key::T, false, Modifiers::LOGO),
        key(Key::Logo, false, Modifiers::NONE),
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super held for a chord is not a tap"
    );

    harness.frame(vec![
        key(Key::Ctrl, true, Modifiers::CTRL),
        key(
            Key::Logo,
            true,
            Modifiers {
                ctrl: true,
                logo: true,
                ..Modifiers::NONE
            },
        ),
        key(Key::Logo, false, Modifiers::CTRL),
        key(Key::Ctrl, false, Modifiers::NONE),
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super added to a held Ctrl is not a tap"
    );

    harness.frame(vec![
        key(Key::Logo, true, Modifiers::LOGO),
        Event::PointerButton {
            pos: pos2(50.0, 50.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::LOGO,
        },
        key(Key::Logo, false, Modifiers::NONE),
        Event::PointerButton {
            pos: pos2(50.0, 50.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super held for a click is not a tap"
    );

    harness.frame(vec![key(Key::Logo, false, Modifiers::NONE)]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "a release with no press is not a tap"
    );
}
