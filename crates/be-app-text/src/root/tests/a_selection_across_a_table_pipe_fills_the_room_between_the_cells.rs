use super::*;

/// Selecting the pipe between two cells fills from just right of the first cell's content to
/// just left of the second's, the grid line between them included.
#[test]
fn a_selection_across_a_table_pipe_fills_the_room_between_the_cells() {
    let mut editor = editor("|a|b|\n|-|-|\n");
    editor.click("text.surface");
    editor.key(Key::Home, Modifiers::CTRL);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::SHIFT);

    editor.snapshot("a_selection_across_a_table_pipe_fills_the_room_between_the_cells");
}
