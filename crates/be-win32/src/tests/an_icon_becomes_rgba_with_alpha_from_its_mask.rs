use super::*;

#[test]
fn an_icon_becomes_rgba_with_alpha_from_its_mask() {
    let with_alpha = [10, 20, 30, 128, 1, 2, 3, 0];
    assert_eq!(
        icon::from_bgra(2, 1, &with_alpha, None).unwrap().rgba,
        [30, 20, 10, 128, 3, 2, 1, 0]
    );
    let without_alpha = [10, 20, 30, 0, 1, 2, 3, 0];
    let mask = [0, 0, 0, 0, 255, 255, 255, 0];
    assert_eq!(
        icon::from_bgra(2, 1, &without_alpha, Some(&mask))
            .unwrap()
            .rgba,
        [30, 20, 10, 255, 3, 2, 1, 0]
    );
    assert_eq!(
        icon::from_bgra(2, 1, &without_alpha, None).unwrap().rgba,
        [30, 20, 10, 255, 3, 2, 1, 255]
    );
    assert_eq!(icon::from_bgra(2, 2, &without_alpha, None), None);

    let mut premultiplied = Image {
        width: 1,
        height: 1,
        rgba: vec![64, 0, 128, 128],
    };
    icon::unpremultiply(&mut premultiplied);
    assert_eq!(premultiplied.rgba, [127, 0, 255, 128]);
}
