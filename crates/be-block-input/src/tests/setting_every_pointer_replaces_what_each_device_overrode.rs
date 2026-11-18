use super::*;

use crate::input_settings::{InputDevice, PointerSetting};

#[test]
fn setting_every_pointer_replaces_what_each_device_overrode() {
    let device = |name: &str| InputDevice {
        name: name.to_owned(),
        vendor: 1,
        product: 2,
    };
    let (mouse, touchpad) = (device("Mouse"), device("Touchpad"));
    let settings = InputSettingsContent::default();
    let root = settings.root();
    let settings = edited(
        &settings,
        [root.set_device_pointer(&mouse, PointerSetting::NaturalScroll(Some(false)))],
    );
    let root = settings.root();
    let settings = edited(
        &settings,
        [root.set_device_pointer(&mouse, PointerSetting::TapToClick(Some(false)))],
    );
    let root = settings.root();
    assert_eq!(root.pointer(&mouse).natural_scroll, Some(false));
    assert_eq!(root.pointer(&touchpad).natural_scroll, None);

    let settings = edited(
        &settings,
        [root.set_every_pointer(PointerSetting::NaturalScroll(Some(true)))],
    );
    let root = settings.root();
    assert_eq!(root.pointer(&mouse).natural_scroll, Some(true));
    assert_eq!(root.pointer(&touchpad).natural_scroll, Some(true));
    assert_eq!(
        root.pointer(&mouse).tap_to_click,
        Some(false),
        "only the setting that was changed for every pointer is replaced"
    );

    let settings = edited(
        &settings,
        [root.set_device_pointer(&touchpad, PointerSetting::NaturalScroll(Some(false)))],
    );
    let root = settings.root();
    assert_eq!(root.pointer(&touchpad).natural_scroll, Some(false));
    assert_eq!(root.pointer(&mouse).natural_scroll, Some(true));
}
