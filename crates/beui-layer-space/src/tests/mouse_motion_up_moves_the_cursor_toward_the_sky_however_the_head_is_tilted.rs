use std::f32::consts::FRAC_PI_2;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::head::{Fov, View, Views};

fn azimuth(direction: Vec3) -> f32 {
    direction.x.atan2(-direction.z)
}

fn elevation(direction: Vec3) -> f32 {
    direction.y.asin()
}

fn moves_up_and_across(space: &mut Space, now: Instant, case: &str) {
    send(space, now, [Input::PointerMoved(centre())]);
    let per_pixel = 1.0 / Head::pixels_per_radian(PLATFORM);
    let before = space.cursor_ray().unwrap().direction;
    send(space, now, [Input::PointerMotion(vec2(0.0, -40.0))]);
    let after = space.cursor_ray().unwrap().direction;
    assert!(
        (azimuth(after) - azimuth(before)).abs() < 1e-4,
        "{case}: up keeps it on its line to the sky"
    );
    assert!(
        (elevation(after) - elevation(before) - 40.0 * per_pixel).abs() < 1e-4,
        "{case}: and raises it toward the sky: {} to {}",
        elevation(before),
        elevation(after)
    );
    send(space, now, [Input::PointerMotion(vec2(60.0, 0.0))]);
    let across = space.cursor_ray().unwrap().direction;
    assert!(
        (elevation(across) - elevation(after)).abs() < 1e-4,
        "{case}: across keeps its height"
    );
    assert!(
        azimuth(across) - azimuth(after) > 50.0 * per_pixel,
        "{case}: and turns it right about the vertical"
    );
}

#[test]
fn mouse_motion_up_moves_the_cursor_toward_the_sky_however_the_head_is_tilted() {
    for (roll, pitch, yaw) in [(0.5, 0.0, 0.0), (0.0, -0.6, 0.0), (FRAC_PI_2, 0.4, 1.0)] {
        let (mut space, now) = space();
        (space.head.roll, space.head.pitch, space.head.yaw) = (roll, pitch, yaw);
        moves_up_and_across(
            &mut space,
            now,
            &format!("roll {roll} pitch {pitch} yaw {yaw}"),
        );
    }

    let (mut space, now) = space();
    let tilted = Head {
        roll: -0.7,
        pitch: 0.3,
        yaw: -0.4,
        ..Head::default()
    };
    space.set_views(Some(Views {
        display_time: now,
        views: vec![View {
            position: tilted.position,
            rotation: tilted.rotation(),
            fov: Fov::from_angles(-0.8, 0.8, 0.8, -0.8),
            size: PLATFORM,
        }],
    }));
    moves_up_and_across(&mut space, now, "a tilted headset");

    let (mut space, now, context) = painted_space();
    space.head.roll = 0.5;
    send(&mut space, now, [Input::PointerMoved(centre())]);
    send(&mut space, now, [Input::PointerMotion(vec2(0.0, -120.0))]);
    let windows = [window(&context, WindowId::MAIN, PLATFORM, "Main")];
    assert_scene(
        &mut space,
        now,
        &windows,
        "mouse_motion_up_moves_the_cursor_toward_the_sky_however_the_head_is_tilted",
    );
}
