use block_editor_beui::ViewChange;
use block_editor_beui::beui::TouchPhase;

use super::*;

#[test]
fn holding_a_square_and_drawing_with_another_finger_draws_an_arrow() {
    let mut editor = editor(CHESS.to_vec());
    let held = editor.rect_of("game.tile.0.0").center();
    let g1 = editor.rect_of("game.tile.6.7").center();
    let f3 = editor.rect_of("game.tile.5.5").center();
    editor.take_view_changes();

    editor.fingers(TouchPhase::Start, &[(1, held)]);
    editor.run();
    editor.fingers(TouchPhase::Start, &[(2, g1)]);
    editor.run();
    editor.fingers(TouchPhase::Move, &[(2, f3)]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(2, f3)]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, held)]);
    editor.run();

    assert!(moves(&editor).is_empty());
    assert!(
        !editor
            .take_view_changes()
            .iter()
            .any(|change| matches!(change, ViewChange::Zoom { .. })),
        "drawing with a second finger does not zoom"
    );
    editor.snapshot("holding_a_square_and_drawing_with_another_finger_draws_an_arrow");
}
