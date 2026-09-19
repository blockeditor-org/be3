use super::*;

#[test]
fn a_pixel_is_found_from_where_the_pointer_is() {
    let canvas = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(80.0, 40.0));

    assert_eq!(pixel_at(Pos2::new(11.0, 21.0), canvas, 8, 4), Some((0, 0)));
    assert_eq!(pixel_at(Pos2::new(89.0, 59.0), canvas, 8, 4), Some((7, 3)));
    assert_eq!(pixel_at(Pos2::new(9.0, 21.0), canvas, 8, 4), None);
}
