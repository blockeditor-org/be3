use super::*;
use crate::ForwardedInput;
use crate::KeyAt;
use crate::primitives::*;
use crate::reactive::*;

fn key(key: Key, pressed: bool) -> Input {
    Input::Key(beui::Keystroke {
        key: Some(key),
        pressed,
        ..Default::default()
    })
}

#[test]
fn super_tapped_alone_runs_its_tap_action_and_still_reaches_the_app() {
    let _root = crate::reactive::RootScope::new();
    let taps = create_signal(0);
    let forwarded = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
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
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    forwarded.update(|forwarded| forwarded.clear());

    harness.frame(vec![
        Input::Modifiers(Modifiers::LOGO),
        key(Key::Logo, true),
    ]);
    harness.frame(vec![
        Input::Modifiers(Modifiers::NONE),
        key(Key::Logo, false),
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super pressed and let go alone is a tap"
    );
    assert!(
        forwarded
            .get_untracked()
            .contains(&Event::key(beui::KeyPress {
                key: Key::Logo,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            })),
        "the app focused still gets the Super it saw pressed let go"
    );

    harness.frame(vec![
        Input::Modifiers(Modifiers::LOGO),
        key(Key::Logo, true),
        key(Key::T, true),
        key(Key::T, false),
        Input::Modifiers(Modifiers::NONE),
        key(Key::Logo, false),
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super held for a chord is not a tap"
    );

    harness.frame(vec![
        Input::Modifiers(Modifiers::CTRL),
        key(Key::Ctrl, true),
        Input::Modifiers(Modifiers {
            ctrl: true,
            logo: true,
            ..Modifiers::NONE
        }),
        key(Key::Logo, true),
        Input::Modifiers(Modifiers::CTRL),
        key(Key::Logo, false),
        Input::Modifiers(Modifiers::NONE),
        key(Key::Ctrl, false),
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super added to a held Ctrl is not a tap"
    );

    harness.frame(vec![
        Input::Modifiers(Modifiers::LOGO),
        key(Key::Logo, true),
        Input::PointerButton {
            pos: pos2(50.0, 50.0),
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::Modifiers(Modifiers::NONE),
        key(Key::Logo, false),
        Input::PointerButton {
            pos: pos2(50.0, 50.0),
            button: PointerButton::Primary,
            pressed: false,
        },
    ]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "Super held for a click is not a tap"
    );

    harness.frame(vec![key(Key::Logo, false)]);
    assert_eq!(
        taps.get_untracked(),
        1,
        "a release with no press is not a tap"
    );
}
