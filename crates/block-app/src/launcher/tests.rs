use super::*;

mod super_alone_is_a_tap_and_super_with_another_key_is_not;

fn key(code: u32, pressed: bool) -> Event {
    Event::PhysicalKey { code, pressed }
}
