use beui::Key;

use super::editor;

#[test]
fn switching_to_hex_view_shows_the_bytes() {
    let mut editor = editor("hello").with_top_bar(false);

    editor.click("editor.menu");
    editor.run();
    for key in [Key::ArrowDown, Key::Enter] {
        editor.key_press(key);
        editor.run();
    }

    editor.snapshot("switching_to_hex_view_shows_the_bytes");
}
