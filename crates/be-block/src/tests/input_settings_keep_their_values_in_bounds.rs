use super::*;

#[test]
fn input_settings_keep_their_values_in_bounds() {
    let settings = InputSettingsContent::default();
    let root = settings.root();
    assert_eq!(root.repeat_delay.milliseconds(), 600);
    assert_eq!(root.repeat_rate.per_second(), 25);
    assert_eq!(root.pointer_speed.get(), 0.0);
    assert_eq!(root.tap_to_click, None);
    assert!(!root.natural_scroll);
    assert_eq!(root.keyboard_layout, "");

    let settings = edited(
        &settings,
        [
            InputSettings::set_keyboard_layout(" de,us "),
            InputSettings::set_repeat_delay(5),
            InputSettings::set_repeat_rate(1000),
            InputSettings::set_pointer_speed(f32::NAN),
            InputSettings::set_tap_to_click(false),
            InputSettings::set_natural_scroll(true),
        ],
    );
    let root = settings.root();
    assert_eq!(root.keyboard_layout, "de,us");
    assert_eq!(root.repeat_delay.milliseconds(), 150);
    assert_eq!(root.repeat_rate.per_second(), 100);
    assert_eq!(root.pointer_speed.get(), 0.0);
    assert_eq!(root.tap_to_click, Some(false));
    assert!(root.natural_scroll);

    let settings = edited(&settings, [InputSettings::set_pointer_speed(-4.0)]);
    assert_eq!(settings.root().pointer_speed.get(), -1.0);
}
