use super::*;

#[test]
fn resetting_a_layout_returns_it_to_the_default() {
    let mut editor = editor(Vec::new());
    editor.click("input.layout");
    editor.type_text("de");

    reset(&mut editor, "input.layout");

    assert_eq!(content(&editor).root().keyboard_layout, None);
}
