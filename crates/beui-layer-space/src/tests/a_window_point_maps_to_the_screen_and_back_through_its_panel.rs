use super::*;
use beui_runner::Layer;

#[test]
fn a_window_point_maps_to_the_screen_and_back_through_its_panel() {
    let (space, _) = space();
    let at = pos2(120.0, 80.0);
    let (below, on_screen) = space
        .down(WindowId::MAIN, at)
        .expect("the main panel is in view");
    assert_eq!(below, WindowId::MAIN, "the platform shows one window");
    assert_eq!(
        on_screen,
        screen(&space, main(&space).world(at)),
        "the point is where the panel's point is drawn, in the platform's points"
    );
    let (window, back) = space
        .up(below, on_screen)
        .expect("the ray through the point hits the panel");
    assert_eq!(window, WindowId::MAIN);
    assert!(
        (back - at).length() < 0.01,
        "the point maps back to where it started: {back:?}"
    );
    assert_eq!(
        space.up(WindowId::MAIN, pos2(1.0, 1.0)),
        None,
        "a point beside every panel reaches no window"
    );
}
