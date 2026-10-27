use super::editor;
use block_ui_test::prelude::*;

#[test]
fn switching_to_hex_view_shows_the_bytes() {
    let mut editor = editor("hello").with_top_bar(false);

    editor.pick_menu("text.hex");

    editor.snapshot("switching_to_hex_view_shows_the_bytes");
}
