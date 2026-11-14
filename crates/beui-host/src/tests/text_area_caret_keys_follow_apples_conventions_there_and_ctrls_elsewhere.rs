use super::*;
use crate::unstyled::text_area::keys::{CaretKey, caret_key};
use beui_text_editor_core::{CursorLeftRightStop, LRDirection, UDDirection};

fn press(key: Key, modifiers: Modifiers) -> KeyPress {
    KeyPress::down(key, modifiers)
}

#[test]
fn text_area_caret_keys_follow_apples_conventions_there_and_ctrls_elsewhere() {
    use CursorLeftRightStop::{Document, Line, Word};
    use LRDirection::{Left, Right};
    let alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    let ctrl = Modifiers::CTRL;
    let both = |key, modifiers| {
        let press = press(key, modifiers);
        (caret_key(press, true), caret_key(press, false))
    };
    let across = |direction, stop| Some(CaretKey::Across(direction, stop));

    // Everywhere: Home and End go to the line's ends, and with Ctrl (Cmd) to the document's.
    for apple in [true, false] {
        let key = |key, modifiers| caret_key(press(key, modifiers), apple);
        assert_eq!(key(Key::Home, Modifiers::NONE), across(Left, Line));
        assert_eq!(key(Key::End, Modifiers::NONE), across(Right, Line));
        assert_eq!(key(Key::Home, ctrl), across(Left, Document));
        assert_eq!(key(Key::End, ctrl), across(Right, Document));
        assert_eq!(key(Key::ArrowLeft, alt), across(Left, Word));
        assert_eq!(key(Key::ArrowRight, alt), across(Right, Word));
    }

    // Cmd+Left and Cmd+Right go to the line's ends on Apple's systems; Ctrl goes by word elsewhere.
    assert_eq!(
        both(Key::ArrowLeft, ctrl),
        (across(Left, Line), across(Left, Word))
    );
    assert_eq!(
        both(Key::ArrowRight, ctrl),
        (across(Right, Line), across(Right, Word))
    );

    // Cmd+Up and Cmd+Down go to the document's ends; Ctrl moves a row elsewhere.
    assert_eq!(
        both(Key::ArrowUp, ctrl),
        (across(Left, Document), Some(CaretKey::Row(UDDirection::Up)))
    );
    assert_eq!(
        both(Key::ArrowDown, ctrl),
        (
            across(Right, Document),
            Some(CaretKey::Row(UDDirection::Down))
        )
    );
}
