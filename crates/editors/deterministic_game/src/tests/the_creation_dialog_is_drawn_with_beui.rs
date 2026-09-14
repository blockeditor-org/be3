use super::*;

#[test]
fn the_creation_dialog_is_drawn_with_beui() {
    let mut editor = creation_editor();

    editor.rect_of("game.choose");
    editor.rect_of("game.selection");
    editor.snapshot("the_creation_dialog_is_drawn_with_beui");
}
