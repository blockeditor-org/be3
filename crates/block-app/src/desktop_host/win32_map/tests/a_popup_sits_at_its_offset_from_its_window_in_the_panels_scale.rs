use super::*;

#[test]
fn a_popup_sits_at_its_offset_from_its_window_in_the_panels_scale() {
    let frame = Rect::new(100, 200, 900, 800);
    let menu = Rect::new(80, 300, 280, 700);
    let panel = Vec2::new(400.0, 300.0);
    assert_eq!(
        popup_rect(frame, menu, panel),
        block_plugin_api::Rect::from_min_size(Pos2::new(-10.0, 50.0), Vec2::new(100.0, 200.0))
    );
    assert_eq!(
        popup_point(frame, menu, Vec2::new(100.0, 200.0), Pos2::new(50.0, 100.0)),
        Point::new(80, 300)
    );
    assert_eq!(popup_rect(Rect::default(), menu, panel).size(), Vec2::ZERO);
}
