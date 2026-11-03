use super::*;
use crate::KeyAt;
use crate::reactive::*;

fn key(key: Key, pressed: bool) -> Input {
    Input::Key(beui::Keystroke {
        key: Some(key),
        pressed,
        ..Default::default()
    })
}

#[test]
fn super_held_over_a_key_the_host_intercepted_is_not_a_tap() {
    let _root = crate::reactive::RootScope::new();
    let taps = create_signal(0);
    let document = build_node(move || {
        Action::new("test.tap", "Tap", move || taps.update(|taps| *taps += 1))
            .shortcut(Chord::tap(Key::Logo))
            .at(KeyAt::Window)
            .capture()
            .register();
        view! {
            <Frame />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![
        Input::Modifiers(Modifiers::LOGO),
        key(Key::Logo, true),
        Input::InterceptedKey {
            key: Key::F,
            pressed: true,
            repeat: false,
        },
        Input::InterceptedKey {
            key: Key::F,
            pressed: false,
            repeat: false,
        },
        Input::Modifiers(Modifiers::NONE),
        key(Key::Logo, false),
    ]);
    assert_eq!(
        taps.get_untracked(),
        0,
        "a key the host took for this program while Super was held makes Super a chord, not a tap"
    );

    harness.frame(vec![
        Input::Modifiers(Modifiers::LOGO),
        key(Key::Logo, true),
        key(Key::F, true),
        key(Key::F, false),
        Input::Modifiers(Modifiers::NONE),
        key(Key::Logo, false),
    ]);
    assert_eq!(
        taps.get_untracked(),
        0,
        "a key nothing took while Super was held makes it a chord too"
    );
}
