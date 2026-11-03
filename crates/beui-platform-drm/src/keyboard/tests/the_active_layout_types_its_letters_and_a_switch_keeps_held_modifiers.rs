use super::*;

fn typed(keyboard: &mut Keyboard, code: u32) -> Option<String> {
    let pressed = keyboard.key(code, true);
    keyboard.key(code, false);
    pressed.inputs.into_iter().find_map(|input| match input {
        Input::Key(Keystroke { text, .. }) => text,
        _ => None,
    })
}

#[test]
fn the_active_layout_types_its_letters_and_a_switch_keeps_held_modifiers() {
    let two = InputConfig {
        layout: "us,de".to_owned(),
        active_layout: 1,
        ..InputConfig::default()
    };
    let mut keyboard = Keyboard::new(&two).expect("the keymap compiles");
    assert_eq!(
        typed(&mut keyboard, KEY_Y).as_deref(),
        Some("z"),
        "a keyboard starts on the config's active layout"
    );

    keyboard.set_layout(0);
    assert_eq!(typed(&mut keyboard, KEY_Y).as_deref(), Some("y"));

    keyboard.key(SHIFT, true);
    keyboard.set_layout(1);
    assert_eq!(
        typed(&mut keyboard, KEY_Y).as_deref(),
        Some("Z"),
        "Shift held across the switch still shifts"
    );
    keyboard.key(SHIFT, false);

    keyboard.set_layout(2);
    assert_eq!(
        typed(&mut keyboard, KEY_Y).as_deref(),
        Some("z"),
        "a layout past the last is no switch"
    );
}
