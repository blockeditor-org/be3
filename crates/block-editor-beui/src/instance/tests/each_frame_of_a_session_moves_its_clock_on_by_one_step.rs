use super::*;
use block_editor_plugin::editor_session::FRAME_STEP;

#[test]
fn each_frame_of_a_session_moves_its_clock_on_by_one_step() {
    let mut session = docked_session();
    session.frame(&[(ViewId::MAIN, 0)]);
    let first = block_editor_plugin::frame_time().expect("a frame sets the frame time");
    let wall = block_editor_plugin::wall_clock();
    session.frame(&[(ViewId::MAIN, 0)]);
    session.frame(&[(ViewId::MAIN, 0)]);
    let third = block_editor_plugin::frame_time().expect("a frame sets the frame time");
    assert_eq!(third - first, FRAME_STEP * 2);
    assert_eq!(block_editor_plugin::wall_clock() - wall, FRAME_STEP * 2);
}
