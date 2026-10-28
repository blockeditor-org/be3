use super::*;
use block_editor_beui::beui::reactive::Get;

#[test]
fn choosing_a_tool_marks_it_in_the_sidebar() {
    let mut editor = editor();

    assert!(pressed(&editor, "Pencil"));
    assert!(!pressed(&editor, "Eraser"));

    editor.click("pixel-art.tool.Eraser");

    assert!(pressed(&editor, "Eraser"));
    assert!(!pressed(&editor, "Pencil"));
}

fn pressed(editor: &BeuiTest<PixelArtApp>, tool: &str) -> bool {
    let node = editor
        .document()
        .find_test_id(&format!("pixel-art.tool.{tool}"))
        .expect("the tool has a button");
    editor.document().api::<ToggleApi>(node).checked().get()
}
