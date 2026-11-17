use be_sdk_beui::beui::{PointerButton, vec2};

use super::*;

#[test]
fn a_carried_piece_snaps_onto_a_square_it_can_move_to() {
    let mut editor = editor(CHESS.to_vec());
    let from = editor.rect_of("game.tile.4.6").center();
    let to = editor.rect_of("game.tile.4.4");
    // Held off the middle of e4, the pawn is drawn on it; between squares it follows the
    // pointer.
    editor.down(from, PointerButton::Primary);
    editor.hover(from + vec2(0.0, -20.0));
    editor.hover(to.center() + vec2(10.0, 8.0));
    let lifted = editor.rect_of("game.lifted");
    assert_eq!(
        lifted.center(),
        to.center(),
        "over a square it can move to, the piece rests on it"
    );
    editor.snapshot("a_carried_piece_snaps_onto_a_square_it_can_move_to");
    editor.up(to.center() + vec2(10.0, 8.0), PointerButton::Primary);
    assert_eq!(moves(&editor).len(), 1, "let go there, it moves");
}
