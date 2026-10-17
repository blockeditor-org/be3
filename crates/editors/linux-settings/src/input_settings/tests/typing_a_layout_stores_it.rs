use super::*;

#[test]
fn typing_a_layout_stores_it() {
    let mut editor = editor(Vec::new());

    editor.click("input-settings.layout");
    editor.run();
    editor.text("de");
    editor.run();

    assert_eq!(
        content(&editor).root().keyboard_layout.as_deref(),
        Some("de")
    );
    editor.snapshot("typing_a_layout_stores_it");
}
