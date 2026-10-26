use super::*;

#[test]
fn a_point_outside_an_entry_is_held_to_its_edge() {
    let frame = PixelRect::from_edges(0, 0, 1920, 1080);
    let size = vec2(1920.0, 1080.0);
    assert_eq!(
        frame.point_at(size, pos2(-5.0, 2000.0)),
        PixelPoint::new(0, 1079)
    );
    assert_eq!(
        frame.point_at(size, pos2(1920.0, 1080.0)),
        PixelPoint::new(1919, 1079)
    );
    assert_eq!(
        frame.point_at(vec2(0.0, 0.0), pos2(10.0, 10.0)),
        PixelPoint::new(0, 0)
    );
}
