use super::editor;

#[test]
fn switching_to_hex_view_shows_the_bytes() {
    let mut editor = editor("hello");

    editor.click("text.hex-view");
    editor.run();

    editor.snapshot("switching_to_hex_view_shows_the_bytes");
}
