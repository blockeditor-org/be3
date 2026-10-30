use super::*;
use block_editor_beui::be_block::ThemeKind;

#[test]
fn a_new_e_paper_tablet_is_set_up_with_only_the_settings_that_differ() {
    let mut setup = SetUp::new(
        DeviceClass::EpaperTablet,
        DisplaySettings {
            theme: Some(ThemeKind::EPaper),
            ..DisplaySettings::default()
        },
    );
    assert!(
        setup.shows("setup.done"),
        "a profile in use with kept defaults opens to be set up"
    );
    assert!(setup.shows("setup.differs.scale"));
    assert!(setup.shows("setup.differs.rotate-in-bar"));
    assert!(setup.shows("setup.differs.keyboard-in-bar"));
    assert!(
        !setup.shows("setup.differs.theme"),
        "a default that matches every device is not listed"
    );
    assert!(
        !setup.shows("settings-profile.use"),
        "the rest of the profile page waits until the setup is done"
    );
    setup.editor.snapshot("setting_up_an_e_paper_tablet");

    setup.editor.click("setup.done");
    setup.editor.run();
    assert!(!setup.shows("setup.done"));
    assert!(
        setup.shows("setup.kind"),
        "the kind stays on the profile page"
    );
    assert!(
        setup.shows("setup.differs.scale"),
        "and so do the differences"
    );
    assert_eq!(setup.here(), DeviceClass::EpaperTablet.display_settings());
}
