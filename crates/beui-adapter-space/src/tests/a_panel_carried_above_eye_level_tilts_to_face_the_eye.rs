use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_panel_carried_above_eye_level_tilts_to_face_the_eye() {
    let (mut space, now) = space();
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    let up = centre() - vec2(0.0, 250.0);
    send(&mut space, now, [Input::PointerMoved(up)]);
    send(&mut space, later(now, TURN_SECONDS), []);
    let panel = main(&space);
    assert!(panel.pose.position.y > space.head.position.y + 0.1);
    assert!(
        panel.normal().y < -0.1,
        "above the eye it tips its face down: {:?}",
        panel.normal()
    );
    assert!(facing_eye(&space, panel) < 1e-3, "toward the eye");
    assert!(level(panel) < 1e-4, "with its edges level");

    let down = centre() + vec2(0.0, 250.0);
    send(&mut space, later(now, 0.5), [Input::PointerMoved(down)]);
    send(&mut space, later(now, 0.6), [button(down, false)]);
    let panel = main(&space);
    assert!(panel.normal().y > 0.1, "below it, up: {:?}", panel.normal());
    assert!(facing_eye(&space, panel) < 1e-3);
    assert!(level(panel) < 1e-4);
}
