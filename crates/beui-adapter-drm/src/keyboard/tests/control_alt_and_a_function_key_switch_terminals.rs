use super::*;

#[test]
fn control_alt_and_a_function_key_switch_terminals() {
    let mut keyboard = Keyboard::new().expect("the default keymap compiles");
    keyboard.key(CONTROL, true);
    keyboard.key(ALT, true);
    let pressed = keyboard.key(KEY_F2, true);

    assert_eq!(pressed.terminal, Some(2));
    assert_eq!(pressed.repeat, None);
    assert!(!pressed.quit, "switching terminals is not quitting");
}
