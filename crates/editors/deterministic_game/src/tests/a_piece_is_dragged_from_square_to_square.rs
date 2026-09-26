use super::*;

#[test]
fn a_piece_is_dragged_from_square_to_square() {
    let mut editor = editor(CHESS.to_vec());
    editor.snapshot("the_chess_board_before_the_first_move");

    let from = editor.rect_of("game.tile.4.6").center();
    let to = editor.rect_of("game.tile.4.4").center();
    editor.drag(from, to);
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].actor, ACCOUNT);
    assert_eq!(editor.label("game.history.0.0"), "e4");
    editor.snapshot("a_piece_is_dragged_from_square_to_square");
}
