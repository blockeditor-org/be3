use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_carried_panel_moved_off_what_it_is_over_returns_smoothly_to_its_distance() {
    let (mut space, now) = space();
    carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let carry = |space: &Space| {
        let carry = space.mouse.carry.as_ref().unwrap();
        (carry.at, carry.distance)
    };
    let (pulled, wanted) = carry(&space);
    assert!(
        pulled < wanted - 0.01,
        "over the main panel it is pulled in"
    );

    send(&mut space, now, steer(true));
    let mut last = pulled;
    for _ in 0..40 {
        send(&mut space, now, [Input::PointerMotion(vec2(20.0, 0.0))]);
        let (at, _) = carry(&space);
        assert!(
            (at - last).abs() < 0.03,
            "turning it off the main panel eases it back, never jumping: {last} then {at}"
        );
        last = at;
    }
    send(&mut space, now, steer(false));
    let (at, still) = carry(&space);
    assert_eq!(still, wanted);
    assert!(
        (at - wanted).abs() < 1e-4,
        "away from every other panel it is back at its distance: {at} {wanted}"
    );
}
