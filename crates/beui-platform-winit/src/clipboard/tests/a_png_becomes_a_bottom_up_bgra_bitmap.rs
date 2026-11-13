use super::*;

#[test]
fn a_png_becomes_a_bottom_up_bgra_bitmap() {
    let mut png = Vec::new();
    let (top, bottom) = ([10, 20, 30, 255], [40, 50, 60, 128]);
    PngEncoder::new(&mut png)
        .write_image(
            &[top, bottom].concat(),
            1,
            2,
            ExtendedColorType::Rgba8,
        )
        .expect("the PNG encodes");
    let dib = dib(&png).expect("the PNG decodes");
    assert_eq!(dib.len(), 40 + 2 * 4);
    assert_eq!(&dib[0..4], 40u32.to_le_bytes(), "a BITMAPINFOHEADER");
    assert_eq!(&dib[4..8], 1i32.to_le_bytes(), "the width");
    assert_eq!(&dib[8..12], 2i32.to_le_bytes(), "the height, positive: bottom row first");
    assert_eq!(&dib[14..16], 32u16.to_le_bytes(), "32 bits a pixel");
    assert_eq!(&dib[40..], [60, 50, 40, 128, 30, 20, 10, 255], "BGRA, the bottom row first");
}
