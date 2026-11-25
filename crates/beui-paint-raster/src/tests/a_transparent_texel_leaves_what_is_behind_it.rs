use super::*;

#[test]
fn a_transparent_texel_leaves_what_is_behind_it() {
    let clear = crate::render(&triangle([255, 255, 255, 0]), 0).unwrap();
    assert_eq!(clear.get_pixel(1, 1).0, [0, 0, 0, 255]);

    let half = crate::render(&triangle([255, 255, 255, 128]), 0).unwrap();
    assert_eq!(half.get_pixel(1, 1).0, [128, 128, 128, 255]);
}
