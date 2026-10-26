use super::*;
use crate::carry::{IN_FRONT, TURN_SECONDS};

#[test]
fn a_carried_panel_stays_in_front_of_the_panel_it_is_carried_over_and_after_release() {
    let (mut space, now) = space();
    let window = carry_popped_out(&mut space, now);
    let wanted = space.carry.as_ref().unwrap().distance;
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let main_depth = main(&space)
        .hit(space.head.ray(PLATFORM, centre().to_vec2()))
        .unwrap()
        .distance;
    assert!(
        wanted > main_depth,
        "at the distance it was picked up from it would be behind the main panel: {wanted} {main_depth}"
    );
    let gaps = gaps_to(&space, window, WindowId::MAIN);
    assert!(!gaps.is_empty());
    assert!(
        gaps.iter().all(|gap| *gap >= IN_FRONT - 1e-3),
        "carried over the main panel, all of it is in front by the margin: {gaps:?}"
    );
    assert!(
        gaps.iter().any(|gap| (*gap - IN_FRONT).abs() < 1e-3),
        "and no nearer than it needs: {gaps:?}"
    );
    let at = space.carry.as_ref().unwrap().at;
    let ray = space.head.ray(PLATFORM, centre().to_vec2());
    assert_eq!(space.pick(ray).map(|hit| hit.window), Some(window));

    let held = space.panel(window).unwrap().pose;
    send(&mut space, later(now, 1.0), [button(centre(), false)]);
    assert_eq!(space.carry, None);
    let released = space.panel(window).unwrap().pose;
    assert!(
        close(released.position, held.position)
            && released.rotation.angle_between(held.rotation) < 1e-3,
        "letting go leaves it where it was drawn"
    );
    assert_eq!(
        space.pick(ray).map(|hit| hit.window),
        Some(window),
        "so the main panel's centre still picks it, by depth alone"
    );
    assert!(
        gaps_to(&space, window, WindowId::MAIN)
            .iter()
            .all(|gap| *gap >= IN_FRONT - 1e-3),
        "not so near the main panel that the two fight for depth"
    );
    assert!(at < wanted);
}
