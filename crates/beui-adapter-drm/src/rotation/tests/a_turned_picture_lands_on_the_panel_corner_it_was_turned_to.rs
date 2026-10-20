use super::*;

#[test]
fn a_turned_picture_lands_on_the_panel_corner_it_was_turned_to() {
    let picture = vec2(30.0, 40.0);
    let top_left = Pos2::ZERO;
    let top_right = pos2(30.0, 0.0);

    assert_eq!(Rotation::Normal.to_panel(picture, top_left), pos2(0.0, 0.0));
    assert_eq!(
        Rotation::Clockwise90.to_panel(picture, top_left),
        pos2(40.0, 0.0),
        "a quarter turn clockwise puts the picture's top along the panel's right edge"
    );
    assert_eq!(
        Rotation::Clockwise90.to_panel(picture, top_right),
        pos2(40.0, 30.0)
    );
    assert_eq!(
        Rotation::Clockwise180.to_panel(picture, top_left),
        pos2(30.0, 40.0)
    );
    assert_eq!(
        Rotation::Clockwise270.to_panel(picture, top_left),
        pos2(0.0, 30.0),
        "three quarter turns put its top along the panel's left edge"
    );
    assert_eq!(
        Rotation::Clockwise270.to_panel(picture, top_right),
        pos2(0.0, 0.0)
    );
    assert_eq!(Rotation::Clockwise90.panel_size(picture), vec2(40.0, 30.0));
    assert_eq!(Rotation::Clockwise90.turned((40, 30)), (30, 40));
    assert_eq!(Rotation::Clockwise180.turned((40, 30)), (40, 30));
}
