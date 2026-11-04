use super::*;

#[test]
fn displays_side_by_side_make_one_picture() {
    let picture = compose(&[
        Shown {
            rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(4.0, 2.0)),
            scale: 1.0,
            picture: filled(4, 2, RED),
        },
        Shown {
            rect: Rect::from_min_size(pos2(4.0, 1.0), vec2(2.0, 2.0)),
            scale: 1.0,
            picture: filled(2, 2, BLUE),
        },
    ])
    .unwrap();

    assert_eq!((picture.width(), picture.height()), (6, 3));
    assert_eq!(pixel(&picture, 3, 1), RED);
    assert_eq!(pixel(&picture, 4, 1), BLUE);
    assert_eq!(pixel(&picture, 5, 2), BLUE);
    assert_eq!(
        pixel(&picture, 0, 2),
        [0, 0, 0, 0],
        "what no display covers is transparent"
    );
    assert_eq!(pixel(&picture, 4, 0), [0, 0, 0, 0]);
}
