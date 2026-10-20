use super::*;

use crate::display_settings::{DisplayMode, Rotation};

#[test]
fn a_screen_back_at_its_default_mode_is_forgotten() {
    let fast = DisplayMode {
        width: 2560,
        height: 1440,
        refresh_millihertz: 239_970,
    };
    let monitor = "DEL|DELL AW2524H|7XQ2B34";
    let settings = DisplaySettingsContent::default();
    assert_eq!(settings.root().mode(monitor), None);

    let settings = edited(&settings, [settings.root().set_mode(monitor, Some(fast))]);
    assert_eq!(settings.root().mode(monitor), Some(fast));
    assert_eq!(
        settings.root().mode("DP-2"),
        None,
        "each monitor has its own"
    );

    let settings = edited(
        &settings,
        [settings
            .root()
            .set_rotation(monitor, Some(Rotation::Clockwise90))],
    );
    let settings = edited(&settings, [settings.root().set_mode(monitor, None)]);
    assert_eq!(settings.root().mode(monitor), None);
    assert_eq!(
        settings.root().rotation(monitor),
        Rotation::Clockwise90,
        "a mode and a rotation are set apart"
    );

    let settings = edited(&settings, [settings.root().set_rotation(monitor, None)]);
    assert!(settings.root().screens.is_empty());
}
