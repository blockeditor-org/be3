use super::*;

#[test]
fn a_turned_rect_covers_the_same_pixels_on_the_panel() {
    let picture = vec2(30.0, 40.0);
    let rect = [2.0, 3.0, 7.0, 11.0];

    assert_eq!(Rotation::Normal.rect_to_panel(picture, rect), rect);
    assert_eq!(
        Rotation::Clockwise90.rect_to_panel(picture, rect),
        [29.0, 2.0, 37.0, 7.0]
    );
    assert_eq!(
        Rotation::Clockwise180.rect_to_panel(picture, rect),
        [23.0, 29.0, 28.0, 37.0]
    );
    assert_eq!(
        Rotation::Clockwise270.rect_to_panel(picture, rect),
        [3.0, 23.0, 11.0, 28.0]
    );
}
