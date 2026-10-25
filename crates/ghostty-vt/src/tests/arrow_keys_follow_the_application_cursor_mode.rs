use crate::{Key, KeyEncoder, KeyInput, Mods, Terminal};

#[test]
fn arrow_keys_follow_the_application_cursor_mode() {
    let mut terminal = Terminal::new(20, 3, 100).unwrap();
    let mut encoder = KeyEncoder::new().unwrap();
    let up = KeyInput {
        key: Key::ARROW_UP,
        mods: Mods::default(),
        text: "",
        unshifted: None,
        pressed: true,
    };

    assert_eq!(encoder.encode(&terminal, &up), b"\x1b[A");
    terminal.write(b"\x1b[?1h");
    assert_eq!(encoder.encode(&terminal, &up), b"\x1bOA");

    let control_c = KeyInput {
        key: Key::C,
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        text: "",
        unshifted: Some('c'),
        pressed: true,
    };
    assert_eq!(encoder.encode(&terminal, &control_c), [3]);

    let letter = KeyInput {
        key: Key::UNIDENTIFIED,
        mods: Mods::default(),
        text: "é",
        unshifted: None,
        pressed: true,
    };
    assert_eq!(encoder.encode(&terminal, &letter), "é".as_bytes());
}
