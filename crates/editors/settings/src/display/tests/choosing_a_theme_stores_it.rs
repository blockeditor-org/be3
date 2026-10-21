use super::*;

#[test]
fn choosing_a_theme_stores_it() {
    let mut settings = Harness::new();
    assert!(settings.says("Default (Dark)"));

    settings.choose(THEME, 1);
    assert_eq!(settings.content().root().theme, Some(ThemeKind::Light));
    settings.editor.snapshot("choosing_a_theme_stores_it");

    settings.choose(THEME, 0);
    assert_eq!(settings.content().root().theme, None);
}
