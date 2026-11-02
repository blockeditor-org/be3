use super::*;

#[test]
fn a_keystroke_prefers_its_physical_key_and_falls_back_to_its_text() {
    let stroke = |key, evdev, text: Option<&str>| Keystroke {
        key,
        pressed: true,
        repeat: false,
        evdev,
        text: text.map(str::to_owned),
    };
    assert_eq!(
        keystroke(&stroke(Some(Key::Q), Some(30), Some("a"))),
        [key(30, true).unwrap()]
    );
    assert_eq!(
        keystroke(&stroke(Some(Key::Q), None, None)),
        [key(16, true).unwrap()]
    );
    assert_eq!(keystroke(&stroke(None, None, Some("é"))), text("é"));
    assert_eq!(keystroke(&stroke(Some(Key::MicMute), None, None)), []);
    assert_eq!(
        keystroke(&Keystroke {
            pressed: false,
            ..stroke(None, None, Some("é"))
        }),
        []
    );
}
