use super::*;
use crate::BESIDE_GAP;
use crate::panel::POINTS_PER_METRE;

#[test]
fn a_window_opened_beside_another_sits_to_its_right() {
    let (mut space, now) = space();
    let (window, _) = open(&mut space, now, WindowPlacement::Beside(WindowId::MAIN));
    let main = main(&space).clone();
    let panel = space.panel(window).unwrap();
    assert!(panel.pose.rotation.angle_between(main.pose.rotation) < 1e-4);
    let right_edge = main.world(pos2(main.size.x, 0.0));
    assert!(
        close(
            panel.world(Pos2::ZERO),
            right_edge + Vec3::X * BESIDE_GAP
        ),
        "its top left corner is just right of the main window's top right one"
    );
    assert!((SMALL.x / POINTS_PER_METRE - panel.metres().x).abs() < 1e-6);
}
