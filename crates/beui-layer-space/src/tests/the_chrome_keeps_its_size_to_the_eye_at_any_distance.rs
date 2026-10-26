use glam::Quat;

use super::the_chrome_around_a_curved_panel::window;
use super::*;

fn button_angle(space: &Space, window: WindowId) -> f32 {
    let panel = space.panel(window).unwrap();
    let metres = panel.metres();
    let layout = chrome::layout(vec2(metres.x, metres.y), panel.chrome, false);
    let (_, rect) = layout.buttons[0];
    let at = |x: f32| {
        let pos = pos2(x / panel.metres_per_point, rect.center().y / panel.metres_per_point);
        panel.world(pos) - space.head.position
    };
    at(rect.max.x).angle_between(at(rect.min.x))
}

fn placed_at(position: Vec3, yaw: f32) -> WindowPlacement {
    let pose = Pose {
        position,
        rotation: Quat::from_rotation_y(yaw),
    };
    WindowPlacement::Host(panel(WindowId::MAIN, pose).placement())
}

#[test]
fn the_chrome_keeps_its_size_to_the_eye_at_any_distance() {
    let (mut space, now, context) = painted_space();
    let eye = space.head.position;
    if let Some(main) = space.panels.first_mut() {
        main.pose.position = eye + Vec3::new(-0.6, 0.05, -1.5);
        main.pose.rotation = Quat::from_rotation_y(0.35);
    }
    let near = open(
        &mut space,
        now,
        placed_at(eye + Vec3::new(0.0, -0.22, -0.6), -0.1),
    )
    .0;
    let far = open(
        &mut space,
        now,
        placed_at(eye + Vec3::new(0.75, 0.45, -3.0), -0.2),
    )
    .0;
    send(&mut space, now, []);

    let (near_size, far_size) = (
        button_angle(&space, near),
        button_angle(&space, far),
    );
    let main_size = button_angle(&space, WindowId::MAIN);
    let similar = |a: f32, b: f32| (a / b).max(b / a) < chrome::KEPT * 1.1;
    assert!(
        similar(near_size, far_size) && similar(near_size, main_size),
        "a button looks as big on a panel 0.6 m away ({near_size}), 3 m away ({far_size}) and on the main panel ({main_size})"
    );
    assert!(
        space.panel(far).unwrap().chrome.scale > 4.0 * space.panel(near).unwrap().chrome.scale,
        "so the far panel's chrome is bigger in the world"
    );
    let far_panel = *space.panel(far).unwrap();
    let bracket = (chrome::GAP + chrome::THICKNESS / 2.0) * far_panel.chrome.scale
        / far_panel.metres_per_point;
    assert_eq!(
        chrome::part_at(&far_panel, pos2(-bracket, -bracket), false),
        Some(Part::Scale(Corner::TopLeft)),
        "and its parts are picked where they are drawn"
    );

    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, near, SMALL, "Near"),
        window(&context, far, SMALL, "Far"),
    ];
    assert_scene(
        &mut space,
        now,
        &windows,
        "the_chrome_keeps_its_size_to_the_eye_at_any_distance",
    );

    let before = button_angle(&space, WindowId::MAIN);
    let held = space.panel(WindowId::MAIN).unwrap().chrome;
    space.head.position.z -= 0.01;
    send(&mut space, now, []);
    assert_eq!(
        space.panel(WindowId::MAIN).unwrap().chrome,
        held,
        "a small step keeps the chrome as it was, so it is not drawn again"
    );
    space.head.position.z += 1.5;
    send(&mut space, now, []);
    let after = button_angle(&space, WindowId::MAIN);
    assert!(
        similar(before, after),
        "walking away keeps the main panel's buttons as big to the eye: {before} then {after}"
    );
}
