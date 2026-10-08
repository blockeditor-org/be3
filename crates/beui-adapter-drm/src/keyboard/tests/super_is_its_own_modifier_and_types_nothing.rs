use super::*;

use beui::Key;

#[test]
fn super_is_its_own_modifier_and_types_nothing() {
    let mut keyboard = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    let held = keyboard.key(LEFT_SUPER, true);
    assert!(held.events.contains(&Event::Modifiers(Modifiers::LOGO)));

    let pressed = keyboard.key(KEY_F, true);
    assert!(pressed.events.contains(&Event::Key {
        key: Key::F,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::LOGO,
    }));
    assert!(
        !pressed
            .events
            .iter()
            .any(|event| matches!(event, Event::Text(_))),
        "Super+F is a shortcut, not a letter"
    );

    keyboard.key(KEY_F, false);
    let released = keyboard.key(LEFT_SUPER, false);
    assert!(released.events.contains(&Event::Modifiers(Modifiers::NONE)));
}
