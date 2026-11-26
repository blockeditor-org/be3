use be_sdk_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn a_chess_move_is_played_with_the_keys_alone() {
    let mut editor = editor(CHESS.to_vec());
    let focused_on = |editor: &BeuiTest<DeterministicGameRoot>, test_id: &str| {
        let document = editor.document();
        document.focused_node().is_some()
            && document.focused_node() == document.find_test_id(test_id)
    };
    let mut reached = false;
    for _ in 0..40 {
        editor.key(Key::Tab, Modifiers::NONE);
        assert!(
            !focused_on(&editor, "game.piece.4.1"),
            "a piece that cannot move is no focus stop"
        );
        if focused_on(&editor, "game.piece.4.6") {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches a movable pawn");
    assert_eq!(
        editor.accessible_label("game.piece.4.6").as_deref(),
        Some("Player 1's pawn, column 5, row 7"),
        "named for its piece and its square"
    );
    editor.snapshot("a_chess_piece_focused_from_the_keyboard");

    editor.key(Key::Enter, Modifiers::NONE);
    editor.key(Key::ArrowUp, Modifiers::NONE);
    editor.key(Key::Enter, Modifiers::NONE);

    let played = moves(&editor);
    assert_eq!(
        played.len(),
        1,
        "the focused pawn is lifted, stepped and dropped by keys"
    );
    let said = editor.label("game.history.0.0");
    assert!(
        said == "e3" || said == "e4",
        "to a square it can reach, not {said}"
    );

    // The pawn is the same item, and its stop follows it to its square.
    let row = if said == "e3" { 5 } else { 4 };
    assert_eq!(
        editor.accessible_label(&format!("game.piece.4.{row}")),
        Some(format!("Player 1's pawn, column 5, row {}", row + 1)),
        "named for the square it moved to"
    );
    assert!(
        editor.document().find_test_id("game.piece.4.6").is_none(),
        "and no longer for the one it left"
    );
}
