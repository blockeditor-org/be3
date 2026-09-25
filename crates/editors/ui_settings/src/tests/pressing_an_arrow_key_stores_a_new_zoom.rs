use super::*;
use block_editor_beui::beui::Key;

#[test]
fn pressing_an_arrow_key_stores_a_new_zoom() {
    let mut settings = Harness::new();

    settings.editor.key_press(Key::Tab);
    settings.run();
    settings.editor.key_press(Key::ArrowRight);
    settings.run();

    assert!(settings.content.root().zoom() > 1.0);
    settings
        .editor
        .snapshot("pressing_an_arrow_key_stores_a_new_zoom");
}
