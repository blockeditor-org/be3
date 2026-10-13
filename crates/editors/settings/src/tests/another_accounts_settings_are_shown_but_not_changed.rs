use super::*;

#[test]
fn another_accounts_settings_are_shown_but_not_changed() {
    let mut editor = editor_for(Uuid::new_v4());

    editor.click("settings.ui-settings");
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
    editor.snapshot("another_accounts_settings_are_shown_but_not_changed");
}
