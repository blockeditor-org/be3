use super::*;

#[test]
fn the_toast_keeps_every_change_on_this_device_only() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]).on_this_device();
    settings.choose(THEME, 1);
    settings.choose(SCALE, 6);
    settings.choose(THEME, 2);
    settings.editor.click(ROTATE_IN_BAR);
    settings.run();
    settings.choose(SCREEN_OFF, 7);
    assert!(
        settings.says("Theme, scale and 2 more now apply on your devices"),
        "one toast names every setting changed, each once"
    );

    settings.editor.click("toast.0.action.keep");
    settings.run();
    let here = settings.here();
    assert_eq!(here.theme, Some(Theme::Dark));
    assert_eq!(here.scale_percent, Some(150));
    assert_eq!(here.rotate_in_bar, Some(true));
    assert_eq!(here.screen_off, Some(ScreenOff::Never));
    assert_eq!(
        settings.every_device(),
        DisplaySettings::default(),
        "every other device is left as it was"
    );
    assert!(!settings.says("now apply"));

    settings.choose(LOCK_AFTER, 0);
    assert!(
        settings.says("Lock now applies on your devices"),
        "a later change starts a new toast"
    );
}
