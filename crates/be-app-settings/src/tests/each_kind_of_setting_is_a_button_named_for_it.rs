use super::*;

#[test]
fn each_kind_of_setting_is_a_button_named_for_it() {
    let editor = editor();
    for (row, name) in [
        ("settings.display", "Display"),
        ("settings.input", "Input"),
        ("settings.default-editors", "Default editors"),
        ("settings.offline", "Offline"),
        ("settings.devices", "Compare devices"),
        ("settings.new-profile", "New settings profile"),
    ] {
        assert_eq!(editor.accessible_label(row).as_deref(), Some(name));
    }
}
