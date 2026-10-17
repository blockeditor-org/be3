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
    for code in 0..0x120 {
        let source = Source::from_code(code);
        assert_eq!(Source::named(&source.name()), Some(source), "{code}");
    }
}
