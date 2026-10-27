use super::*;
use block_editor_beui::beui::PointerButton;

#[test]
fn right_dragging_draws_an_arrow_and_right_clicking_circles_a_square() {
    let mut editor = editor(CHESS.to_vec());
    let e2 = editor.rect_of("game.tile.4.6").center();
    let e4 = editor.rect_of("game.tile.4.4").center();
    let d5 = editor.rect_of("game.tile.3.3").center();

    editor.down(e2, PointerButton::Secondary);
    editor.up(e4, PointerButton::Secondary);
    editor.down(d5, PointerButton::Secondary);
    editor.up(d5, PointerButton::Secondary);

    assert!(moves(&editor).is_empty());
    editor.record();

    editor.click(d5);
    editor.record();
    editor.snapshot("right_dragging_draws_an_arrow_and_right_clicking_circles_a_square");
}
