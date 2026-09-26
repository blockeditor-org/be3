use super::*;
use beui::{Key, Modifiers};

#[test]
fn keys_reach_the_running_program_as_terminal_input() {
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    assert_eq!(key_bytes(Key::C, ctrl), Some(vec![3]));
    assert_eq!(key_bytes(Key::Enter, Modifiers::NONE), Some(b"\r".to_vec()));
    assert_eq!(
        key_bytes(Key::ArrowUp, Modifiers::NONE),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(key_bytes(Key::C, Modifiers::NONE), None);
}
