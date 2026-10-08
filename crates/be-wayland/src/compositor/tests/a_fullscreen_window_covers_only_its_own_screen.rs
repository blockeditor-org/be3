use super::*;

#[test]
fn a_fullscreen_window_covers_only_its_own_screen() {
    let mut harness = Harness::new();
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN.x / 2.0, SCREEN.y));
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness.app.set_screens(vec![left, right]);
    let (window, id) = harness.open();

    window.toplevel.as_ref().unwrap().set_fullscreen(None);
    harness.acknowledge(&window);
    let listed = harness.app.windows().list().get_untracked();
    assert_eq!(
        listed[0].fullscreen,
        Some(left),
        "the window asks for the screen it was shown on"
    );
    assert_eq!(harness.window(id).size(), left.size());
    assert_eq!(
        harness.client.received.size,
        Some((left.width() as i32, left.height() as i32))
    );
}
