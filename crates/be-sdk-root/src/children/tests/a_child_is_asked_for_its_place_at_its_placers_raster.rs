use super::*;

#[test]
fn a_child_is_asked_for_its_place_at_its_placers_raster() {
    // A child is its editor's main view the size of its place, laid out and drawn at the
    // pixels per point of the screen that placed it.
    let mut placement = placed(rect((10.0, 20.0), (210.0, 120.5)), ChildMode::Live);
    let screen = child_screen(&placement, EditorInstanceId(5), 2.0);
    assert_eq!(screen.screen, CHILD_SCREEN);
    assert_eq!(screen.instance, EditorInstanceId(5));
    assert_eq!(screen.view, ViewId::MAIN);
    assert_eq!(
        (screen.metrics.logical.x, screen.metrics.logical.y),
        (200.0, 100.5)
    );
    assert_eq!(
        (screen.metrics.pixel_width, screen.metrics.pixel_height),
        (400, 201)
    );
    assert_eq!((screen.metrics.scale_factor, screen.metrics.raster), (2.0, 2.0));
    assert_eq!(screen.frame, None);

    // Placed with a frame of its own, it draws its chrome there, its anchor in its own points.
    placement.own_frame = true;
    placement.top_bar = TopBar::Shown;
    placement.anchor = Some(rect((10.0, 0.0), (90.0, 20.0)));
    let frame = child_screen(&placement, EditorInstanceId(5), 2.0)
        .frame
        .expect("a child with a frame of its own is asked to draw it");
    assert_eq!(frame.chrome, FrameChrome::Drawn);
    assert_eq!(frame.top_bar, TopBar::Shown);
    assert_eq!(frame.anchor, Some(rect((0.0, -20.0), (80.0, 0.0))));
}
