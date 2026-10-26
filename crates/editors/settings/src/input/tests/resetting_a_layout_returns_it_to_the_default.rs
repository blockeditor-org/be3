use super::*;

#[test]
fn resetting_a_layout_returns_it_to_the_default() {
    let mut editor = editor(Vec::new());
    editor.click("input.layout");
    editor.run();
    editor.type_text("de");
    editor.run();

    reset(&mut editor, "input.layout");

    assert_eq!(content(&editor).root().keyboard_layout, None);
}
