use super::*;

#[test]
fn typing_a_layout_stores_it() {
    let mut editor = editor(Vec::new());

    editor.click("input.layout");
    editor.type_text("de");

    assert_eq!(
        content(&editor).root().keyboard_layout.as_deref(),
        Some("de")
    );
    editor.snapshot("typing_a_layout_stores_it");
}
