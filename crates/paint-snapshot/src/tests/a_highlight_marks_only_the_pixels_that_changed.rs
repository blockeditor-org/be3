use image::{Rgba, RgbaImage};

#[test]
fn a_highlight_marks_only_the_pixels_that_changed() {
    let before = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
    let mut after = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
    after.put_pixel(1, 0, Rgba([255, 255, 255, 255]));

    let highlight = crate::highlight(&before, &after);

    assert_eq!(highlight.changed, 3);
    assert_eq!(highlight.image.get_pixel(0, 0).0, [40, 40, 40, 255]);
    assert_eq!(highlight.image.get_pixel(1, 0).0, [255, 0, 255, 255]);
    assert_eq!(highlight.image.get_pixel(0, 1).0, [255, 0, 255, 255]);
}
