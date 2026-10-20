use super::*;

#[test]
fn a_change_here_goes_to_every_device_and_a_toast_says_so() {
    let mut settings = Harness::new().on_this_device();

    settings.choose(THEME, 1);
    assert_eq!(settings.every_device().theme, Some(Theme::Light));
    assert_eq!(
        settings.here().theme,
        None,
        "this device keeps nothing of its own"
    );
    assert!(settings.says("Theme now applies on your devices"));
    settings
        .editor
        .snapshot("a_change_here_goes_to_every_device_and_a_toast_says_so");
}
