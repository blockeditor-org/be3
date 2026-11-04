use super::*;

#[test]
fn key_names_read_back_as_they_are_written() {
    assert_eq!(key("f24"), Source::Key(194));
    assert_eq!(key("LButton"), Source::Button(0x110));
    assert_eq!(key("Media_Play_Pause"), key("PlayPause"));
    assert_eq!(key("button:0x118"), Source::Button(0x118));
    assert_eq!(Source::Button(0x118).name(), "Button:0x118");
    assert_eq!(
        Source::named("Button:5"),
        None,
        "a button code is past the keys"
    );
    assert_eq!(
        Source::named("Key 0x9e"),
        Some(Source::Key(0x9e)),
        "a code can be written with a space"
    );
    assert_eq!(
        Source::named("key 0x2a8"),
        Some(Source::Button(0x2a8)),
        "codes past the keys' are buttons'"
    );
    assert_eq!(Source::named("Key 0x300"), None, "past every code");
    assert_eq!(Source::named("Keypad"), None);
    assert_eq!(key("LeftCtrl").label(), "Left Ctrl");
    assert_eq!(key("Numpad7").label(), "Numpad 7");
    assert_eq!(key("F13").label(), "F13");
    assert_eq!(key("MouseBack").label(), "Mouse Back");
    assert_eq!(Source::Key(0x9e).label(), "Key 0x9e");
    assert_eq!(Wheel::Up.label(), "Wheel up");
    for code in 0..0x120 {
        let source = Source::from_code(code);
        assert_eq!(Source::named(&source.name()), Some(source), "{code}");
    }
}
