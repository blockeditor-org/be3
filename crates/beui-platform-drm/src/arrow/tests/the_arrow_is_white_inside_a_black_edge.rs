use super::*;

#[test]
fn the_arrow_is_white_inside_a_black_edge() {
    let pixels = pixels();

    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_eq!(pixel(&pixels, 0, 1), [0, 0, 0, 255], "the tip is outlined");
    assert_eq!(
        pixel(&pixels, 2, 8),
        [255, 255, 255, 255],
        "the body is filled"
    );
    assert_eq!(
        pixel(&pixels, 10, 2),
        [0, 0, 0, 0],
        "beside the arrow is clear"
    );
}
