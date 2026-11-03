use super::*;

#[test]
fn the_toast_keeps_every_change_in_the_block_it_makes_for_this_device() {
    let mut settings = Harness::in_a_window().on_this_device_with(None);
    settings.choose(THEME, 2);
    settings.choose(SCALE, 6);
    settings.editor.click(KEYBOARD_IN_BAR);
    settings.run();

    settings.editor.click("toast.0.action.keep");
    settings.run();
    let here = settings.here();
    assert_eq!(here.theme, Some(ThemeKind::Dark));
    assert_eq!(here.scale_percent, Some(150));
    assert_eq!(here.keyboard_in_bar, Some(true));
    assert_eq!(
        settings.every_device(),
        DisplaySettings::default(),
        "every other device is left as it was"
    );
}
