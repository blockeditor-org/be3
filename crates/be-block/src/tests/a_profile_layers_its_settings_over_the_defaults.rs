use super::*;
use crate::display_settings::{DisplayMode, Rotation};
use crate::input_settings::{InputDevice, PointerSetting, PointerSpeed};
use crate::settings::{
    Settings, SettingsContent, SettingsProfile, SettingsProfileContent, layered,
};
use crate::{ChildChange, Root, Theme};

#[test]
fn a_profile_layers_its_settings_over_the_defaults() {
    let kind = DisplaySettingsContent::CONTENT_TYPE;
    let (shared, own, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let settings = edited(
        &SettingsContent::default(),
        [Settings::set_kind(kind, shared)],
    );
    let profile = edited(
        &SettingsProfileContent::default(),
        [SettingsProfile::set_kind(kind, own)],
    );
    assert_eq!(settings.root().layers(kind, None).bottom_up(), [shared]);
    assert_eq!(
        settings
            .root()
            .layers(kind, Some(&profile.root()))
            .bottom_up(),
        [shared, own]
    );
    let replaced = edited(
        &profile,
        profile.root().child_edit(ChildChange::Replace {
            old: own,
            new: copy,
        }),
    );
    assert_eq!(replaced.root().kind(kind), Some(copy));
    let deleted = edited(
        &settings,
        settings.root().child_edit(ChildChange::Delete(shared)),
    );
    assert_eq!(
        deleted
            .root()
            .layers(kind, Some(&profile.root()))
            .bottom_up(),
        [own]
    );

    let defaults = edited(
        &DisplaySettingsContent::default(),
        [DisplaySettings::set_theme(Some(Theme::Light))],
    );
    assert_eq!(
        layered([defaults.root(), DisplaySettings::default()]).theme(),
        Theme::Light
    );
    let dark = edited(
        &defaults,
        [DisplaySettings::set_theme(Some(Theme::OledDark))],
    );
    assert_eq!(
        layered([defaults.root(), dark.root()]).theme(),
        Theme::OledDark
    );

    let screen = "DPI-1";
    let mode = DisplayMode {
        width: 1872,
        height: 1404,
        refresh_millihertz: 85_000,
    };
    let moded = edited(&defaults, [defaults.root().set_mode(screen, Some(mode))]);
    let rotated = edited(
        &DisplaySettingsContent::default(),
        [DisplaySettings::default().set_rotation(screen, Some(Rotation::Clockwise270))],
    );
    let merged = layered([moded.root(), rotated.root()]);
    assert_eq!(
        merged.mode(screen),
        Some(mode),
        "a screen is layered field by field"
    );
    assert_eq!(merged.rotation(screen), Rotation::Clockwise270);

    let mouse = InputDevice {
        name: "Mouse".to_owned(),
        vendor: 1,
        product: 2,
    };
    let trackpad = InputDevice {
        name: "Trackpad".to_owned(),
        vendor: 3,
        product: 4,
    };
    let slow = PointerSetting::Speed(Some(PointerSpeed::new(-0.5)));
    let fast = PointerSetting::Speed(Some(PointerSpeed::new(0.5)));
    let lower = edited(
        &InputSettingsContent::default(),
        [
            InputSettings::default().set_device_pointer(&mouse, slow),
            InputSettings::set_repeat_rate(Some(40)),
        ],
    );
    let upper = edited(
        &InputSettingsContent::default(),
        [InputSettings::default().set_every_pointer(fast)],
    );
    let merged = layered([lower.root(), upper.root()]);
    assert_eq!(
        merged.pointer(&mouse).speed,
        Some(PointerSpeed::new(0.5)),
        "every pointer in a profile wins over one pointer in the defaults"
    );
    assert_eq!(
        merged.pointer(&trackpad).speed,
        Some(PointerSpeed::new(0.5))
    );
    assert_eq!(
        merged.repeat_rate(),
        40,
        "what the profile leaves unset is inherited"
    );
}
