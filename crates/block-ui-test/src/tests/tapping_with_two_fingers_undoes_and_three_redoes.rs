use super::*;

use beui::{TouchPhase, pos2};
use block_editor_beui::BlockCommand;

#[test]
fn tapping_with_two_fingers_undoes_and_three_redoes() {
    let (mut test, block) = undoable_editor();
    test.set_histories([(
        block,
        block_editor_beui::BlockHistory {
            can_undo: true,
            can_redo: true,
        },
    )]);
    test.run();
    let spots = [pos2(200.0, 300.0), pos2(260.0, 300.0), pos2(320.0, 300.0)];

    for fingers in [2, 3] {
        for (finger, spot) in spots.iter().take(fingers).enumerate() {
            test.finger(finger as u64 + 1, TouchPhase::Start, *spot);
        }
        test.next_frame();
        for (finger, spot) in spots.iter().take(fingers).enumerate() {
            test.finger(finger as u64 + 1, TouchPhase::End, *spot);
        }
        test.run();
    }

    assert_eq!(
        test.take_block_commands(),
        [(block, BlockCommand::Undo), (block, BlockCommand::Redo)]
    );
}
