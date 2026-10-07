use super::*;

#[test]
fn resetting_tap_to_click_returns_it_to_the_device_default() {
    let mut editor = editor();
    assert_eq!(content(&editor).root().tap_to_click, None);

    editor.click("input-settings.tap-to-click");
    editor.run();
    assert_eq!(content(&editor).root().tap_to_click, Some(true));

    editor.click("input-settings.tap-to-click");
    editor.run();
    assert_eq!(content(&editor).root().tap_to_click, Some(false));

    editor.click("input-settings.tap-to-click.reset");
    editor.run();
    assert_eq!(content(&editor).root().tap_to_click, None);

    editor.click("input-settings.tap-to-click.reset");
    editor.run();
    editor.click("input-settings.tap-to-click");
    editor.run();
    assert_eq!(content(&editor).root().tap_to_click, Some(true));
    editor.snapshot("resetting_tap_to_click_returns_it_to_the_device_default");
}
