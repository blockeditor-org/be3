use super::*;

/// A caret before a pipe stands just right of the content on its left, and after it just left
/// of the content on its right, never on the grid line between them.
#[test]
fn a_caret_beside_a_table_pipe_stands_by_the_cells_content() {
    let mut editor = editor("|a|b|\n|-|-|\n");
    editor.click("text.surface");
    editor.key(Key::Home, Modifiers::CTRL);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.snapshot("a_caret_beside_a_table_pipe_stands_by_the_cells_content_before");

    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.snapshot("a_caret_beside_a_table_pipe_stands_by_the_cells_content_after");
}
