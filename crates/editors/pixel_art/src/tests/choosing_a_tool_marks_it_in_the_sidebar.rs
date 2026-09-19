use super::*;

#[test]
fn choosing_a_tool_marks_it_in_the_sidebar() {
    let (mut editor, _) = editor();

    assert!(pressed(&editor, "Pencil"));
    assert!(!pressed(&editor, "Eraser"));

    editor.click("pixel-art.tool.Eraser");
    editor.run();

    assert!(pressed(&editor, "Eraser"));
    assert!(!pressed(&editor, "Pencil"));
}

fn pressed(editor: &BeuiTest<PixelArtApp>, tool: &str) -> bool {
    let node = editor
        .document()
        .find_test_id(&format!("pixel-art.tool.{tool}"))
        .expect("the tool has a button");
    toggle_button_pressed(editor.document(), node)
}
