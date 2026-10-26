use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_panel_carried_far_behind_another_stays_behind_it() {
    let (mut space, now, context) = painted_space();
    let window = open_ahead(&mut space, now, 0.5);
    let grab = pos2(120.0, 120.0);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab), button(grab, true)],
    );
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [Input::PointerMoved(grab + vec2(60.0, 40.0))]);
    let carry = space.mouse.carry.as_ref().expect("the main panel is carried");
    assert!(
        (carry.at - carry.distance).abs() < 1e-4,
        "a panel far nearer than the carried one does not pull it forward: {} {}",
        carry.at,
        carry.distance
    );
    let ray = space.head.ray(PLATFORM, centre().to_vec2());
    assert_eq!(
        space.pick(ray).map(|hit| hit.window),
        Some(window),
        "so the nearer window stays in front of it"
    );

    let windows = [
        super::the_chrome_around_a_curved_panel::window(&context, WindowId::MAIN, PLATFORM, "Main"),
        super::the_chrome_around_a_curved_panel::window(&context, window, SMALL, "Near"),
    ];
    assert_scene(
        &mut space,
        now,
        &windows,
        "a_panel_carried_far_behind_another_stays_behind_it",
    );
}
