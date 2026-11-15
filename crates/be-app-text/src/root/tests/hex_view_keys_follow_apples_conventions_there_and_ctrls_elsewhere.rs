use beui::{Key, KeyPress, Modifiers};

use crate::hex::hex_target;

#[test]
fn hex_view_keys_follow_apples_conventions_there_and_ctrls_elsewhere() {
    let len = 100;
    let current = 40;
    let target =
        |key, modifiers, apple| hex_target(KeyPress::down(key, modifiers), current, len, apple);
    let ctrl = Modifiers::CTRL;
    for apple in [true, false] {
        assert_eq!(target(Key::Home, ctrl, apple), Some(0));
        assert_eq!(target(Key::End, ctrl, apple), Some(len));
        assert_eq!(target(Key::Home, Modifiers::NONE, apple), Some(32));
        assert_eq!(target(Key::End, Modifiers::NONE, apple), Some(47));
    }
    assert_eq!(target(Key::ArrowUp, ctrl, true), Some(0));
    assert_eq!(target(Key::ArrowDown, ctrl, true), Some(len));
    assert_eq!(target(Key::ArrowLeft, ctrl, true), Some(32));
    assert_eq!(target(Key::ArrowRight, ctrl, true), Some(47));
    assert_eq!(target(Key::ArrowUp, ctrl, false), Some(24));
    assert_eq!(target(Key::ArrowDown, ctrl, false), Some(56));
    assert_eq!(target(Key::ArrowLeft, ctrl, false), Some(39));
    assert_eq!(target(Key::ArrowRight, ctrl, false), Some(41));
}
