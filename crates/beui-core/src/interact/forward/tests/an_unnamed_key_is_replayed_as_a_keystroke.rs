use super::*;

#[test]
fn an_unnamed_key_is_replayed_as_a_keystroke() {
    let mut replay = Replay::default();
    assert_eq!(
        replayed(
            &mut replay,
            vec![key(Key::Unidentified, Modifiers::NONE, Some("ş"))],
            1.0
        ),
        [Input::Key(Keystroke {
            key: Some(Key::Unidentified),
            pressed: true,
            repeat: false,
            evdev: Some(30),
            text: Some("ş".to_owned()),
        })]
    );
    let press = KeyPress {
        key: Key::Unidentified,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(
        replay.intercepted(Some(press), Modifiers::NONE),
        [],
        "an intercepted key needs a name to be sent on"
    );
}
