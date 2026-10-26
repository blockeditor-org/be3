use super::*;
use crate::OVER_NEARER;
use crate::panel::POINTS_PER_METRE;

#[test]
fn a_window_opened_over_another_sits_where_that_window_would_show_it_a_little_in_front() {
    let (mut space, now) = space();
    let at = pos2(64.0, 48.0);
    let (window, _) = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at,
        },
    );
    let main = main(&space).clone();
    let panel = space.panel(window).unwrap();
    assert!(
        panel.pose.rotation.angle_between(main.pose.rotation) < 1e-4,
        "it faces the way the main panel faces"
    );
    assert!(
        close(
            panel.world(Pos2::ZERO),
            main.world(at) + main.normal() * OVER_NEARER
        ),
        "its top left corner is in front of the main panel's point {at:?}"
    );
    assert!(
        close(
            panel.world(pos2(SMALL.x, SMALL.y)),
            main.world(at + SMALL) + main.normal() * OVER_NEARER
        ),
        "and its bottom right corner over the point its size away"
    );
    assert!((SMALL.x / POINTS_PER_METRE - panel.metres().x).abs() < 1e-6);
}
