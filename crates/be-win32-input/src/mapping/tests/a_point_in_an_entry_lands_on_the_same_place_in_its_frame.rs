use super::*;

#[test]
fn a_point_in_an_entry_lands_on_the_same_place_in_its_frame() {
    let frame = PixelRect::new(-1920, 100, 800, 600);
    let size = vec2(400.0, 300.0);
    assert_eq!(
        frame.point_at(size, pos2(0.0, 0.0)),
        PixelPoint::new(-1920, 100)
    );
    assert_eq!(
        frame.point_at(size, pos2(200.0, 150.0)),
        PixelPoint::new(-1520, 400)
    );
    assert_eq!(
        frame.point_at(size, pos2(399.9, 299.9)),
        PixelPoint::new(-1121, 699)
    );
}
