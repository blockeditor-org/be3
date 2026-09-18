use super::*;
use block_editor_plugin::beui::Key;

#[test]
fn pressing_an_arrow_key_stores_a_new_zoom() {
    let (mut editor, block) = editor();

    editor.key_press(Key::Tab);
    editor.run();
    editor.key_press(Key::ArrowRight);
    editor.run();

    assert!(block.read().unwrap().zoom() > 1.0);
    editor.snapshot("pressing_an_arrow_key_stores_a_new_zoom");
}
