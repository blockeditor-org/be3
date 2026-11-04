use super::*;

#[test]
fn a_display_at_a_smaller_scale_is_enlarged_to_the_largest() {
    let mut halves = filled(2, 1, RED);
    halves.rgba[4..8].copy_from_slice(&BLUE);
    let picture = compose(&[
        Shown {
            rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(2.0, 2.0)),
            scale: 2.0,
            picture: filled(4, 4, BLUE),
        },
        Shown {
            rect: Rect::from_min_size(pos2(2.0, 0.0), vec2(2.0, 1.0)),
            scale: 1.0,
            picture: halves,
        },
    ])
    .unwrap();

    assert_eq!(
        (picture.width(), picture.height()),
        (8, 4),
        "the desktop at the largest scale"
    );
    assert_eq!(pixel(&picture, 3, 3), BLUE);
    assert_eq!(
        pixel(&picture, 4, 0),
        RED,
        "each pixel of the smaller display covers four"
    );
    assert_eq!(pixel(&picture, 5, 1), RED);
    assert_eq!(pixel(&picture, 6, 1), BLUE);
    assert_eq!(pixel(&picture, 6, 2), [0, 0, 0, 0]);
}
