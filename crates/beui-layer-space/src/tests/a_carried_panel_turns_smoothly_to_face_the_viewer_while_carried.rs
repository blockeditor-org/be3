use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_carried_panel_turns_smoothly_to_face_the_viewer_while_carried() {
    let (mut space, now) = space();
    let turned = glam::Quat::from_rotation_y(0.6);
    space.panels[0].pose.rotation = turned;
    let grab = centre() + vec2(-60.0, -40.0);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab), button(grab, true)],
    );
    let grabbed = on(&space, WindowId::MAIN, grab);
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    send(&mut space, now, []);
    assert!(
        main(&space).pose.rotation.angle_between(turned) < 1e-4,
        "it starts at the rotation it had"
    );

    let facing = |space: &Space| facing_eye(space, main(space));
    let start = facing(&space);
    let mut last = start;
    for step in 1..=4 {
        let at = later(now, TURN_SECONDS * step as f32 / 5.0);
        send(&mut space, at, []);
        let angle = facing(&space);
        assert!(
            angle < last && angle > 1e-3,
            "it turns a little each frame, not all at once: {last} then {angle}"
        );
        assert!(
            (on(&space, WindowId::MAIN, grab) - grabbed).length() < 0.5,
            "about the grabbed point, which stays under the cursor"
        );
        last = angle;
    }
    let done = later(now, TURN_SECONDS);
    send(&mut space, done, []);
    assert!(facing(&space) < 1e-3, "then it faces the viewer");
    assert!(level(main(&space)) < 1e-4, "with its edges level");

    let moved = grab + vec2(250.0, 80.0);
    send(&mut space, later(done, 0.1), [Input::PointerMoved(moved)]);
    assert!(
        facing(&space) < 1e-3,
        "and goes on facing the viewer as it is carried"
    );
    let carried = main(&space).pose;
    send(&mut space, later(done, 0.2), [button(moved, false)]);
    assert_eq!(space.carry, None);
    let released = main(&space).pose;
    assert!(
        close(released.position, carried.position)
            && released.rotation.angle_between(carried.rotation) < 1e-3,
        "so letting go has nothing left to turn: {carried:?} then {released:?}"
    );
}
