use super::*;

#[test]
fn choosing_a_theme_stores_it() {
    let mut settings = Harness::new();
    assert!(settings.says("Default (Dark)"));

    settings.choose(1);
    assert_eq!(settings.content().root().theme, Some(Theme::Light));
    settings.editor.snapshot("choosing_a_theme_stores_it");

    settings.choose(0);
    assert_eq!(settings.content().root().theme, None);
}
