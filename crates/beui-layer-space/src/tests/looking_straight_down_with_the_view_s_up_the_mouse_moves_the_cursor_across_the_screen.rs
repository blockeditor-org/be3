use std::f32::consts::FRAC_PI_2;

use super::*;
use crate::tuning::Up;

fn moved(space: &mut Space, now: Instant, motion: Vec2) -> Vec2 {
    let before = space.cursor_screen().unwrap();
    send(space, now, [Input::PointerMotion(motion)]);
    space.cursor_screen().unwrap() - before
}

#[test]
fn looking_straight_down_with_the_view_s_up_the_mouse_moves_the_cursor_across_the_screen() {
    for roll in [0.0, 0.6] {
        let (mut space, now) = space();
        space.tuning.simulator = Up::View;
        (space.head.pitch, space.head.roll, space.head.yaw) = (-FRAC_PI_2, roll, 0.8);
        send(&mut space, now, [Input::PointerMoved(centre())]);

        let up = moved(&mut space, now, vec2(0.0, -40.0));
        assert!(
            up.x.abs() < 0.5 && (up.y + 40.0).abs() < 1.0,
            "roll {roll}: up moves it up the screen: {up:?}"
        );
        let right = moved(&mut space, now, vec2(60.0, 0.0));
        assert!(
            (right.x - 60.0).abs() < 2.0 && right.y.abs() < 2.0,
            "roll {roll}: right moves it right: {right:?}"
        );
        send(&mut space, now, [Input::PointerMotion(vec2(-60.0, 40.0))]);
        assert!(
            (space.cursor_screen().unwrap() - centre()).length() < 0.5,
            "roll {roll}: and back to straight down"
        );

        send(&mut space, now, [Input::PointerMotion(vec2(0.0, -100.0))]);
        send(&mut space, now, [Input::PointerMotion(vec2(0.0, 200.0))]);
        let below = space.cursor_screen().unwrap() - centre();
        assert!(
            below.x.abs() < 0.5 && (below.y - 101.0).abs() < 2.0,
            "roll {roll}: it passes straight under the feet: {below:?}"
        );
    }

    let (mut space, now) = space();
    space.head.pitch = -FRAC_PI_2;
    send(&mut space, now, [Input::PointerMoved(centre())]);
    send(&mut space, now, [Input::PointerMotion(vec2(0.0, -100.0))]);
    send(&mut space, now, [Input::PointerMotion(vec2(0.0, 200.0))]);
    let below = space.cursor_screen().unwrap() - centre();
    assert!(
        (below.y - 101.0).abs() > 20.0,
        "with the sky's up, it cannot pass under the feet: {below:?}"
    );
}
