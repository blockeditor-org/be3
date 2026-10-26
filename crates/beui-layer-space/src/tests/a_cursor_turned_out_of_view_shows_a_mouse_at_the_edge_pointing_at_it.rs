use std::f32::consts::FRAC_PI_2;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::cursor::AWAY_INSET;
use crate::head::Ray;

#[test]
fn a_cursor_turned_out_of_view_shows_a_mouse_at_the_edge_pointing_at_it() {
    let (mut space, now, context) = painted_space();
    let at = centre() + vec2(-200.0, 0.0);
    send(&mut space, now, [Input::PointerMoved(at)]);
    let saved = space.cursor().expect("the cursor is placed");

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(400.0, 0.0))]);
    send(&mut space, now, steer(false));
    assert_eq!(
        space.cursor(),
        Some(saved),
        "turning leaves it at its point"
    );
    assert_eq!(space.cursor_at(), None, "which is out of view");
    let (marker, angle) = space.cursor_away().expect("a marker shows where it went");
    let drawn = screen(&space, marker);
    assert!(
        (drawn.x - AWAY_INSET).abs() < 0.5 && (drawn.y - centre().y).abs() < 0.5,
        "turned right, the marker sits inside the left edge: {drawn:?}"
    );
    assert!(
        (angle + FRAC_PI_2).abs() < 1e-3,
        "its arrow points left, toward the cursor: {angle}"
    );
    let windows = [window(&context, WindowId::MAIN, PLATFORM, "Main")];
    assert!(space.scene(&windows, &view(now)).cursor.is_some());
    assert_scene(
        &mut space,
        now,
        &windows,
        "a_cursor_turned_out_of_view_shows_a_mouse_at_the_edge_pointing_at_it",
    );

    let got = delivered(&send(
        &mut space,
        now,
        [button(at, true), button(at, false)],
    ));
    let eye = space.head.position;
    let aimed = main(&space)
        .hit(Ray {
            origin: eye,
            direction: (saved - eye).normalize(),
        })
        .expect("the cursor is over the main panel")
        .pos;
    assert!(
        got.iter()
            .all(|(window, input)| *window == WindowId::MAIN
                && (pos_of(input) - aimed).length() < 0.01),
        "a click goes where the cursor is, not where the marker is: {got:?}"
    );

    let off = space.head.rotation().inverse() * (saved - eye);
    let pixels = (-off.x).atan2(-off.z) * Head::pixels_per_radian(PLATFORM);
    send(&mut space, now, [Input::PointerMotion(vec2(pixels, 0.0))]);
    assert!(space.cursor_away().is_none(), "the mouse brings it back");
    assert!(
        (space.cursor_screen().unwrap() - centre()).length() < 0.5,
        "to the middle of the view: {:?}",
        space.cursor_screen()
    );

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(1000.0, 0.0))]);
    send(&mut space, now, steer(false));
    let (marker, angle) = space.cursor_away().expect("behind, it still shows");
    assert!(
        (screen(&space, marker).x - AWAY_INSET).abs() < 0.5 && (angle + FRAC_PI_2).abs() < 1e-3,
        "on the side to turn toward"
    );
}
