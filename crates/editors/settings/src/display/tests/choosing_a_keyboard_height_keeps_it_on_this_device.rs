use super::*;

#[test]
fn choosing_a_keyboard_height_keeps_it_on_this_device() {
    let mut settings = Harness::new().on_this_device();

    settings.choose(KEYBOARD, 5);
    assert_eq!(settings.here().keyboard_percent, Some(115));
    assert_eq!(settings.here().keyboard_scale(), 1.15);
    assert_eq!(settings.every_device().keyboard_percent, None);

    settings.choose(KEYBOARD, 0);
    assert_eq!(settings.here().keyboard_percent, None);
}
