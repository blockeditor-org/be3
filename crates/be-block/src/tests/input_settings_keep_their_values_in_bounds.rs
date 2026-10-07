use super::*;

use crate::input_settings::{InputDevice, PointerSettings, PointerSpeed};

#[test]
fn input_settings_keep_their_values_in_bounds() {
    let mouse = InputDevice {
        name: "Mouse".to_owned(),
        vendor: 0x046d,
        product: 0xc52b,
    };
    let settings = InputSettingsContent::default();
    let root = settings.root();
    assert_eq!(root.keyboard_layout, None);
    assert_eq!(root.repeat_delay(), 600);
    assert_eq!(root.repeat_rate(), 25);
    assert_eq!(root.pointer(&mouse), PointerSettings::default());

    let settings = edited(
        &settings,
        [
            InputSettings::set_keyboard_layout(Some(" de,us ")),
            InputSettings::set_repeat_delay(Some(5)),
            InputSettings::set_repeat_rate(Some(1000)),
            InputSettings::set_pointer(
                &mouse,
                PointerSettings {
                    speed: Some(PointerSpeed::new(-4.0)),
                    tap_to_click: Some(false),
                    natural_scroll: None,
                },
            ),
        ],
    );
    let root = settings.root();
    assert_eq!(root.keyboard_layout.as_deref(), Some("de,us"));
    assert_eq!(root.repeat_delay(), 150);
    assert_eq!(root.repeat_rate(), 100);
    let pointer = root.pointer(&mouse);
    assert_eq!(pointer.speed.map(PointerSpeed::get), Some(-1.0));
    assert_eq!(pointer.tap_to_click, Some(false));
    assert_eq!(pointer.natural_scroll, None);

    let settings = edited(
        &settings,
        [
            InputSettings::set_keyboard_layout(None),
            InputSettings::set_repeat_delay(None),
            InputSettings::set_pointer(&mouse, PointerSettings::default()),
        ],
    );
    let root = settings.root();
    assert_eq!(root.keyboard_layout, None);
    assert_eq!(root.repeat_delay(), 600);
    assert!(
        root.pointers.is_empty(),
        "a device back at its defaults is forgotten"
    );
}
