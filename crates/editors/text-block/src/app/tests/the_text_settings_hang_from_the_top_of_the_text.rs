use super::editor;

#[test]
fn the_text_settings_hang_from_the_top_of_the_text() {
    let mut editor = editor("hello").with_top_bar(false);

    editor.pick_menu("text.settings");

    editor.snapshot("the_text_settings_hang_from_the_top_of_the_text");
}
