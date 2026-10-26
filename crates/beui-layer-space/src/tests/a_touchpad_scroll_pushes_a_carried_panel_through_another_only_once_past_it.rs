use super::*;
use crate::carry::{IN_FRONT, TURN_SECONDS};

#[test]
fn a_touchpad_scroll_pushes_a_carried_panel_through_another_only_once_past_it() {
    let (mut space, now) = space();
    let window = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let ray = space.head.ray(PLATFORM, centre().to_vec2());
    let hit = |space: &Space| space.pick(ray).map(|hit| hit.window);
    assert_eq!(hit(&space), Some(window));
    let held = space.panel(window).unwrap().pose;

    let nudge = Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 2.0)));
    send(&mut space, now, [nudge.clone()]);
    assert_eq!(
        hit(&space),
        Some(window),
        "a small push farther keeps it in front of the main panel"
    );
    assert!(
        close(space.panel(window).unwrap().pose.position, held.position),
        "where it was held"
    );

    let mut pushes = 1;
    while hit(&space) == Some(window) {
        send(&mut space, now, [nudge.clone()]);
        pushes += 1;
        assert!(pushes < 200, "pushing on, it goes through");
    }
    assert!(pushes > 2, "only after several pushes: {pushes}");
    let gaps = gaps_to(&space, window, WindowId::MAIN);
    assert!(
        gaps.iter().all(|gap| *gap <= -IN_FRONT + 1e-3)
            && gaps.iter().all(|gap| *gap > -3.0 * IN_FRONT),
        "to just behind it: {gaps:?}"
    );
}
