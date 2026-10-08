use super::*;

mod super_alone_is_a_tap_and_super_with_another_key_is_not;

fn key(key: Key, pressed: bool, modifiers: beui::Modifiers) -> Event {
    Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers,
    }
}

fn press(key: Key) -> Event {
    self::key(key, true, beui::Modifiers::NONE)
}

fn release(key: Key) -> Event {
    self::key(key, false, beui::Modifiers::NONE)
}

fn held(modifiers: beui::Modifiers) -> Event {
    Event::Modifiers(modifiers)
}
