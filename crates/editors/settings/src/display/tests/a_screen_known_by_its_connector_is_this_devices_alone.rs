use super::*;

#[test]
fn a_screen_known_by_its_connector_is_this_devices_alone() {
    let panel = HostDisplay {
        id: "DP-1".to_owned(),
        ..gaming_monitor()
    };
    let mut settings = Harness::in_a_session(vec![panel]).on_this_device();
    assert!(settings.says("This device only"));

    settings.choose(REFRESH, 1);
    assert_eq!(
        settings.here().mode("DP-1"),
        Some(saved(2560, 1440, 143_998)),
        "a screen with no serial is written to this device's profile"
    );
    assert_eq!(settings.every_device().mode("DP-1"), None);

    settings.open_menu("display.DP-1");
    assert!(
        !settings.editor.shown("display.DP-1.keep"),
        "nothing else could use it, so there is nothing to keep"
    );
    settings.editor.click(format!("{SCREEN}.reset"));
    settings.run();
    assert_eq!(settings.here().mode("DP-1"), None);

    let mut monitor = Harness::in_a_session(vec![gaming_monitor()]).on_this_device();
    monitor.choose(REFRESH, 1);
    assert_eq!(
        monitor.every_device().mode(MONITOR),
        Some(saved(2560, 1440, 143_998)),
        "a monitor known by its serial follows it to every device"
    );
}
