use super::*;

#[test]
fn a_profile_shows_the_theme_it_inherits() {
    let mut settings = Harness::new().in_a_profile_over(ThemeKind::EPaper);

    assert!(
        settings.says("Default (E-paper)"),
        "a profile's own block follows the theme set for every device"
    );
    settings
        .editor
        .snapshot("a_profile_shows_the_theme_it_inherits");
}
