use super::*;
use crate::carry::{IN_FRONT, TURN_SECONDS};

#[test]
fn scrolling_a_carried_panel_farther_than_the_one_behind_puts_it_behind() {
    let (mut space, now) = space();
    let window = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let ray = space.head.ray(PLATFORM, centre().to_vec2());
    let hit = |space: &Space| space.pick(ray).map(|hit| hit.window);
    assert_eq!(hit(&space), Some(window));

    send(&mut space, now, [scroll(1.0)]);
    assert_eq!(
        hit(&space),
        Some(WindowId::MAIN),
        "one step farther takes it through the main panel"
    );
    let gaps = gaps_to(&space, window, WindowId::MAIN);
    assert!(
        gaps.iter().all(|gap| *gap <= -IN_FRONT + 1e-3),
        "to behind it, clear of it: {gaps:?}"
    );

    send(&mut space, now, [scroll(-1.0)]);
    assert_eq!(
        hit(&space),
        Some(window),
        "a step nearer brings it back in front"
    );
    send(&mut space, now, [scroll(1.0)]);
    assert_eq!(hit(&space), Some(WindowId::MAIN));

    send(&mut space, later(now, 0.5), [button(centre(), false)]);
    assert_eq!(space.carry, None);
    assert_eq!(
        hit(&space),
        Some(WindowId::MAIN),
        "and let go there, it stays behind"
    );
}
