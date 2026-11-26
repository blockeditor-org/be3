use be_sdk_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn a_selected_piece_is_carried_to_a_square_from_the_keyboard() {
    let mut editor = editor(CHESS.to_vec());
    let pawn = editor.rect_of("game.tile.4.6").center();
    editor.click(pawn);
    editor.key(Key::Space, Modifiers::NONE);
    editor.key(Key::ArrowUp, Modifiers::NONE);
    editor.key(Key::Enter, Modifiers::NONE);

    let played = moves(&editor);
    assert_eq!(played.len(), 1, "the selected pawn moves from the keyboard");
    let said = editor.label("game.history.0.0");
    assert!(
        said == "e3" || said == "e4",
        "to a square it can reach, not {said}"
    );
}
