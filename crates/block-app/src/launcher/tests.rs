use super::*;

mod super_alone_is_a_tap_and_super_with_another_key_is_not;

fn held(modifiers: Modifiers) -> Event {
    Event::Modifiers(modifiers)
}

fn key(key: beui::Key, pressed: bool) -> Event {
    Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers: Modifiers::LOGO,
    }
}
