use super::*;

#[test]
fn the_demo_paints_its_own_background_in_the_editors_theme() {
    let host = EditorHost::default();
    let mut test = BeuiTest::<BeuiGalleryApp>::new(OwnedEditor::new(host, Uuid::new_v4()))
        .with_size(Vec2::new(1000.0, 700.0));
    test.set_preferences(block_editor_beui::Preferences {
        theme: block_editor_beui::ThemeKind::Light,
        ..Default::default()
    });
    test.run();
    test.snapshot("the_demo_paints_its_own_background_in_the_editors_theme");
}
