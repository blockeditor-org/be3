use super::*;

#[test]
fn the_editor_takes_on_a_theme_chosen_while_it_is_open() {
    let mut test = alone(WIDE, Page::Editor);
    test.context().set_preference_layer(
        beui::PreferenceSource::Developer,
        beui::PreferenceLayer {
            theme: Some(beui::ThemeKind::Light),
            ..beui::PreferenceLayer::EMPTY
        },
    );
    test.frame(Vec::new());
    test.snapshot("editor_light");
}
