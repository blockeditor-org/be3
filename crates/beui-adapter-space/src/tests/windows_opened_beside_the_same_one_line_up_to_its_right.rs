use super::*;
use crate::BESIDE_GAP;

#[test]
fn windows_opened_beside_the_same_one_line_up_to_its_right() {
    let (mut space, now) = space();
    let (first, _) = open(&mut space, now, WindowPlacement::Beside(WindowId::MAIN));
    let (second, _) = open(&mut space, now, WindowPlacement::Beside(WindowId::MAIN));
    let first = space.panel(first).unwrap().clone();
    let second = space.panel(second).unwrap();
    assert!(
        close(
            second.world(Pos2::ZERO),
            first.world(pos2(first.size.x, 0.0)) + Vec3::X * BESIDE_GAP
        ),
        "the second sits right of the first instead of on top of it"
    );
}
