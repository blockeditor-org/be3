use super::*;

use std::time::Duration;

use crate::display_settings::ScreenOff;

#[test]
fn screens_turn_off_after_ten_minutes_unless_told_otherwise() {
    let settings = DisplaySettingsContent::default();
    assert_eq!(
        settings.root().screen_off().after(),
        Some(Duration::from_secs(600))
    );

    let settings = edited(
        &settings,
        [DisplaySettings::set_screen_off(Some(ScreenOff::Never))],
    );
    assert_eq!(settings.root().screen_off(), ScreenOff::Never);
    assert_eq!(settings.root().screen_off().after(), None);

    let settings = edited(
        &settings,
        [DisplaySettings::set_screen_off(Some(ScreenOff::After {
            minutes: 2,
        }))],
    );
    assert_eq!(
        settings.root().screen_off().after(),
        Some(Duration::from_secs(120))
    );
}
