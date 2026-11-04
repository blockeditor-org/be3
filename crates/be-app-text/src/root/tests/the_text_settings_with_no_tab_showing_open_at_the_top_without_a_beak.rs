use super::editor;

#[test]
fn the_text_settings_with_no_tab_showing_open_at_the_top_without_a_beak() {
    let mut editor = editor("hello").with_top_bar(false).in_tab(None);

    editor.pick_menu("text.settings");

    editor.snapshot("the_text_settings_with_no_tab_showing_open_at_the_top_without_a_beak");
}
