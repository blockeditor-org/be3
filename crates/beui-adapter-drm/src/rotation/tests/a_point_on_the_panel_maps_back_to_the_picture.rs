use super::*;

#[test]
fn a_point_on_the_panel_maps_back_to_the_picture() {
    let picture = vec2(30.0, 40.0);
    let at = pos2(4.0, 9.0);
    for rotation in [
        Rotation::Normal,
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::Clockwise270,
    ] {
        let panel = rotation.to_panel(picture, at);
        assert_eq!(rotation.to_picture(picture, panel), at, "{rotation:?}");
    }
    assert_eq!(
        Rotation::Clockwise90.to_picture(picture, pos2(40.0, 0.0)),
        Pos2::ZERO,
        "a touch at the panel's top right corner is the turned picture's top left"
    );
}
