use super::*;

#[test]
fn the_panel_corners_sample_the_turned_picture() {
    assert_eq!(
        Rotation::Normal.corners(),
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
    );
    assert_eq!(
        Rotation::Clockwise90.corners(),
        [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        "the panel's top right shows the picture's top left"
    );
    assert_eq!(
        Rotation::Clockwise180.corners(),
        [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]]
    );
    assert_eq!(
        Rotation::Clockwise270.corners(),
        [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]
    );
}
