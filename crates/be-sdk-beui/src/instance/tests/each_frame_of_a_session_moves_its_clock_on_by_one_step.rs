use super::*;
use be_sdk::editor_session::FRAME_STEP;

#[test]
fn each_frame_of_a_session_moves_its_clock_on_by_one_step() {
    let mut session = docked_session();
    session.frame(&[(ViewId::MAIN, 0)]);
    let first = be_sdk::frame_time().expect("a frame sets the frame time");
    let wall = be_sdk::wall_clock();
    session.frame(&[(ViewId::MAIN, 0)]);
    session.frame(&[(ViewId::MAIN, 0)]);
    let third = be_sdk::frame_time().expect("a frame sets the frame time");
    assert_eq!(third - first, FRAME_STEP * 2);
    assert_eq!(be_sdk::wall_clock() - wall, FRAME_STEP * 2);
}
