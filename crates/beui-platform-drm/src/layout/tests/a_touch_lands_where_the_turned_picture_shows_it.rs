use super::*;

#[test]
fn a_touch_lands_where_the_turned_picture_shows_it() {
    let screen = Rect::from_min_size(pos2(100.0, 0.0), vec2(300.0, 400.0));
    let near_the_panels_top_right = pos2(0.9, 0.1);

    assert_eq!(
        absolute(near_the_panels_top_right, screen, Rotation::Normal),
        pos2(370.0, 40.0)
    );
    assert_eq!(
        absolute(near_the_panels_top_right, screen, Rotation::Clockwise90),
        pos2(130.0, 40.0),
        "the panel's top right is the top left of a picture turned a quarter clockwise"
    );
    assert_eq!(
        absolute(near_the_panels_top_right, screen, Rotation::Clockwise180),
        pos2(130.0, 360.0)
    );
    assert_eq!(
        absolute(near_the_panels_top_right, screen, Rotation::Clockwise270),
        pos2(370.0, 360.0)
    );
}
