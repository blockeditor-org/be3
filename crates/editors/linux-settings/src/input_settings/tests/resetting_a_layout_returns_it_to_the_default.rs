use super::*;

#[test]
fn resetting_a_layout_returns_it_to_the_default() {
    let mut editor = editor(Vec::new());
    editor.click("input-settings.layout");
    editor.run();
    editor.text("de");
    editor.run();

    editor.click("input-settings.layout.reset");
    editor.run();

    assert_eq!(content(&editor).root().keyboard_layout, None);
}
