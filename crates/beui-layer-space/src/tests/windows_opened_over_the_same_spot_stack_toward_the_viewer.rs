use super::*;
use crate::OVER_NEARER;

#[test]
fn windows_opened_over_the_same_spot_stack_toward_the_viewer() {
    let (mut space, now) = space();
    let over = WindowPlacement::Over {
        window: WindowId::MAIN,
        at: pos2(64.0, 48.0),
    };
    let (first, _) = open(&mut space, now, over.clone());
    let (second, _) = open(&mut space, now, over);
    let main = main(&space).clone();
    let first = space.panel(first).unwrap().clone();
    let second = space.panel(second).unwrap();
    assert!(
        close(
            second.world(Pos2::ZERO),
            first.world(Pos2::ZERO) + main.normal() * OVER_NEARER
        ),
        "the second sits in front of the first instead of in its plane"
    );
}
