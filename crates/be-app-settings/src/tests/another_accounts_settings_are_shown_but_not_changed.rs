use super::*;

#[test]
fn another_accounts_settings_are_shown_but_not_changed() {
    let mut editor = editor_for(Uuid::from_u128(0xacc0));

    editor.click("settings.display");
    editor.run();
    editor.snapshot("another_accounts_settings_are_shown_but_not_changed");
    editor.click("settings.devices");
    editor.run();
    editor.click("settings.new-profile");
    editor.run();
    assert!(
        editor.store().created().is_empty(),
        "nothing is made inside someone else's settings"
    );
    assert!(
        editor
            .content::<SettingsContent>(None)
            .root()
            .profiles()
            .is_empty()
    );
}
