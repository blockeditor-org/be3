use super::*;
use crate::carry::{CLOSE, IN_FRONT, TURN_SECONDS};

#[test]
fn a_panel_carried_close_behind_another_comes_in_front_of_it() {
    let (mut space, now) = space();
    let main_depth = main(&space).pose.position.distance(space.head.position);
    let gap = main_depth * CLOSE / 2.0;
    let window = open_ahead(&mut space, now, main_depth - gap);
    let grab = pos2(120.0, 120.0);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab), button(grab, true)],
    );
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    let now = later(now, TURN_SECONDS);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab + vec2(10.0, 0.0))],
    );
    let ray = space.head.ray(PLATFORM, centre().to_vec2());
    assert_eq!(
        space.pick(ray).map(|hit| hit.window),
        Some(WindowId::MAIN),
        "a panel {gap} m behind another, within {CLOSE} of its distance, comes in front"
    );
    let gaps = gaps_to(&space, window, WindowId::MAIN);
    assert!(
        !gaps.is_empty() && gaps.iter().all(|gap| *gap <= -IN_FRONT + 1e-3),
        "by the margin: {gaps:?}"
    );
}
