use super::*;

#[test]
fn a_layout_from_the_config_moves_the_letters() {
    let german = InputConfig {
        layout: "de".to_owned(),
        ..InputConfig::default()
    };
    let mut keyboard = Keyboard::new(&german).expect("the German keymap compiles");

    let pressed = keyboard.key(KEY_Y, true);

    assert!(pressed.events.contains(&Event::Text("z".to_owned())));
}
