use super::*;

#[test]
fn a_scale_no_option_holds_is_shown_as_custom() {
    let mut settings = Harness::new();
    let mut content = DisplaySettingsContent::default();
    content.apply(&DisplaySettings::set_scale_percent(Some(133)));
    settings.editor.hold(None, content);
    settings.run();
    assert!(
        settings.says("Custom (133%)"),
        "a stored scale no option holds is shown, not left blank"
    );
    settings
        .editor
        .snapshot("a_scale_no_option_holds_is_shown_as_custom");

    settings.choose(SCALE, 7);
    assert_eq!(settings.content().root().scale_percent, Some(175));
    assert!(
        !settings.says("Custom"),
        "choosing another option drops the custom one"
    );
}
