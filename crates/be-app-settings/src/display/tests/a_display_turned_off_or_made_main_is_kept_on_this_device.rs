use super::*;
use be_sdk_beui::be_block::display_settings::ScreenUse;

#[test]
fn a_display_turned_off_or_made_main_is_kept_on_this_device() {
    let mut settings = Harness::in_a_session(laptop_beside(gaming_monitor())).on_this_device();

    settings.editor.click("display.DP-1.main");
    settings.run();
    assert_eq!(
        settings.here().main_screen().as_deref(),
        Some(MONITOR),
        "the monitor is made the main display"
    );

    settings.choose("display.eDP-1.use", 1);
    assert_eq!(
        settings.here().used(LAPTOP),
        ScreenUse::Mirror {
            of: MONITOR.to_owned()
        },
        "the laptop shows what the monitor shows"
    );
    settings.choose("display.eDP-1.use", 2);
    assert_eq!(settings.here().used(LAPTOP), ScreenUse::Off);
    assert_eq!(
        settings.every_device().used(LAPTOP),
        ScreenUse::Desktop,
        "a display's use is this device's alone"
    );
    settings
        .editor
        .snapshot("a_display_turned_off_or_made_main_is_kept_on_this_device");

    settings.choose("display.eDP-1.use", 0);
    assert_eq!(settings.here().used(LAPTOP), ScreenUse::Desktop);
    assert!(settings.here().screen(LAPTOP).used.is_none());
}
