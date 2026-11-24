use be_sdk_api::Vec2 as ViewVec2;

use super::*;

#[test]
fn a_child_placed_with_an_intrinsic_size_is_laid_out_at_it_and_drawn_at_its_zoom() {
    // A checklist 200 by 100 points big, placed in place at twice that on a zoomed canvas.
    let mut placement = placed(rect((10.0, 20.0), (410.0, 220.0)), ChildMode::InPlace);
    placement.intrinsic = Some(ViewVec2 { x: 200.0, y: 100.0 });
    let (size, zoom) = placed_zoom(&placement, false).expect("drawn at its place's zoom");
    assert_eq!((size.x, size.y, zoom), (200.0, 100.0, 2.0));

    // It is laid out at its intrinsic size, painted at its place's pixels all the same.
    let screen = child_screen(&placement, EditorInstanceId(5), 1.5, zoom);
    assert_eq!(
        (screen.metrics.logical.x, screen.metrics.logical.y),
        (200.0, 100.0)
    );
    assert_eq!(screen.metrics.raster, 3.0);
    assert_eq!(
        (screen.metrics.pixel_width, screen.metrics.pixel_height),
        (600, 300)
    );

    // Drawn from its view's points to its place: scaled by its zoom.
    let to_placer = placed_transform(&placement, zoom);
    assert_eq!(to_placer.apply(pos2(0.0, 0.0)), pos2(10.0, 20.0));
    assert_eq!(to_placer.apply(pos2(200.0, 100.0)), pos2(410.0, 220.0));

    // An editor that pans and zooms its own world is handed its place instead, at zoom 1, and
    // so is one placed with no intrinsic size.
    assert_eq!(placed_zoom(&placement, true), None);
    placement.intrinsic = None;
    assert_eq!(placed_zoom(&placement, false), None);
}
