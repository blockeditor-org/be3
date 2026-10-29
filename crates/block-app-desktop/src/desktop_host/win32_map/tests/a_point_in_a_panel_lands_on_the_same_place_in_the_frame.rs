use super::*;

#[test]
fn a_point_in_a_panel_lands_on_the_same_place_in_the_frame() {
    let frame = Rect::new(100, 200, 900, 800);
    let panel = Vec2::new(400.0, 300.0);
    assert_eq!(
        frame_point(frame, panel, Pos2::new(200.0, 150.0)),
        Point::new(400, 300)
    );
    assert_eq!(
        frame_point(frame, panel, Pos2::new(0.0, 0.0)),
        Point::new(0, 0)
    );
    assert_eq!(
        frame_point(frame, panel, Pos2::new(500.0, -10.0)),
        Point::new(799, 0)
    );
    assert_eq!(
        frame_point(frame, Vec2::ZERO, Pos2::new(5.0, 5.0)),
        Point::new(0, 0)
    );
}
