use super::*;

fn key(key: Key, pressed: bool) -> Event {
    Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn edit(old: &str, new: &str) -> Vec<Event> {
    let mut events = Vec::new();
    typed(old, new, &mut events);
    events
}

#[test]
fn a_soft_keyboard_edit_types_its_difference() {
    assert_eq!(edit("hel", "help"), [Event::Text("p".to_owned())]);
    assert_eq!(
        edit("teh", "the "),
        [
            key(Key::Backspace, true),
            key(Key::Backspace, false),
            key(Key::Backspace, true),
            key(Key::Backspace, false),
            Event::Text("he ".to_owned()),
        ]
    );
    assert_eq!(
        edit("né", "n"),
        [key(Key::Backspace, true), key(Key::Backspace, false)]
    );
    assert_eq!(
        edit("a", "a\nb"),
        [
            key(Key::Enter, true),
            key(Key::Enter, false),
            Event::Text("b".to_owned()),
        ]
    );
    assert_eq!(
        edit("a", "a\tb"),
        [
            key(Key::Tab, true),
            key(Key::Tab, false),
            Event::Text("b".to_owned()),
        ]
    );
    assert_eq!(edit("a", "a"), []);
}
