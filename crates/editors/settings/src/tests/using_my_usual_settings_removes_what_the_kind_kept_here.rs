use super::*;
use block_editor_beui::be_block::Theme;

#[test]
fn using_my_usual_settings_removes_what_the_kind_kept_here() {
    let mut setup = SetUp::new(
        DeviceClass::EpaperTablet,
        DisplaySettings {
            theme: Some(Theme::EInk),
            ..DisplaySettings::default()
        },
    );
    setup.editor.click("setup.usual");
    setup.editor.run();
    let here = setup.here();
    assert_eq!(here.scale_percent, None);
    assert_eq!(here.rotate_in_bar, None);
    assert_eq!(
        here.theme,
        Some(Theme::EInk),
        "a value that was not listed stays as it is"
    );
    assert_eq!(setup.profile().class, Some(DeviceClass::EpaperTablet));
    assert!(!setup.shows("setup.done"), "nothing is left to set up");
    assert!(!setup.shows("setup.usual"));
    assert!(setup.shows("settings-profile.use") || setup.shows("settings.display"));
}
