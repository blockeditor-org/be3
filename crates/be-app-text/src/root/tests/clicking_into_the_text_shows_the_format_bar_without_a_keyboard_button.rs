use super::editor;

#[test]
fn clicking_into_the_text_shows_the_format_bar_without_a_keyboard_button() {
    let mut editor = editor("word");
    assert!(!editor.shown("text.format.bold"));

    editor.click("text.surface");
    editor.run();

    assert!(editor.shown("text.format.bold"));
    assert!(
        !editor.shown("text.format.done"),
        "a mouse opens no on-screen keyboard to put away"
    );
}
