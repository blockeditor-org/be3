use super::*;

fn turn(panel: &Panel, start: &Panel) -> (Vec3, f32) {
    let (axis, angle) = (panel.pose.rotation * start.pose.rotation.inverse()).to_axis_angle();
    match angle > std::f32::consts::PI {
        true => (-axis, std::f32::consts::TAU - angle),
        false => (axis, angle),
    }
}

fn drag(space: &mut Space, now: Instant, by: Vec2) -> Panel {
    let start = *main(space);
    let grab = on_part(space, WindowId::MAIN, Part::Turn);
    send(
        space,
        now,
        [
            Input::PointerMoved(grab),
            button(grab, true),
            Input::PointerMoved(grab + by),
        ],
    );
    let turned = *main(space);
    send(space, now, [button(grab + by, false)]);
    space.panels[0] = start;
    turned
}

#[test]
fn dragging_the_rotate_control_turns_the_panel_as_seen_from_the_eye_wherever_it_is_on_screen() {
    let heads = [
        ("level", Head::default()),
        (
            "rolled",
            Head {
                roll: std::f32::consts::FRAC_PI_2,
                pitch: -0.5,
                ..Head::default()
            },
        ),
    ];
    for (head_name, head) in heads {
        for (place, x) in [("centre", 0.5), ("left", 0.2), ("right", 0.8)] {
            let (mut space, now) = space();
            space.head = head;
            let at = Head::direction(PLATFORM, vec2(x * PLATFORM.x, 0.5 * PLATFORM.y));
            let panel = &mut space.panels[0];
            panel.pose.position = head.position + head.rotation() * at * 1.5;
            panel.face(head.position, head.up());
            let start = *main(&space);
            let sight = chrome::Sight::new(head.position, start.pose.position, head.rotation());
            let (screen_right, screen_up) = (head.rotation() * Vec3::X, head.up());
            let case = format!("{head_name} head, panel at the {place}");

            let panel = drag(&mut space, now, vec2(60.0, 0.0));
            let (axis, angle) = turn(&panel, &start);
            assert!(angle > 0.05, "{case}: dragging across turns it: {angle}");
            assert!(
                close(panel.pose.position, start.pose.position),
                "{case}: about its centre"
            );
            assert!(
                axis.dot(sight.up) > 0.999,
                "{case}: about the sight's up: {axis:?} {:?}",
                sight.up
            );
            assert!(
                (panel.normal() - start.normal()).dot(screen_right) > 0.05,
                "{case}: its near edge follows the drag to the screen's right"
            );
            assert!(
                (panel.pose.rotation * Vec3::Y).dot(sight.right).abs() < 1e-4,
                "{case}: no roll as seen from the eye"
            );

            let panel = drag(&mut space, now, vec2(0.0, 60.0));
            let (axis, angle) = turn(&panel, &start);
            assert!(angle > 0.05, "{case}: dragging down tilts it: {angle}");
            assert!(
                axis.dot(sight.right) > 0.999,
                "{case}: about the sight's right: {axis:?} {:?}",
                sight.right
            );
            assert!(
                (panel.normal() - start.normal()).dot(screen_up) < -0.05,
                "{case}: its near (top) edge follows the drag down the screen"
            );
            assert!(
                (panel.pose.rotation * Vec3::X).dot(sight.up).abs() < 1e-4,
                "{case}: no roll as seen from the eye"
            );
        }
    }
}
