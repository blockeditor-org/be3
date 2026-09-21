use super::*;

#[test]
fn letters_land_where_their_advances_put_them() {
    let mut fonts = Fonts::new(&FontSources::default());
    let phase = 1.0 / SUBPIXEL_POSITIONS as f32;

    for text in [
        "Automatic",
        "Choices",
        "Reset",
        "Hamburgefonstiv",
        "nnnnnnnn",
    ] {
        for size in [11.0, 12.0, 13.0, 14.0, 16.0, 21.0] {
            let pixel_size = size as u32;
            let shaped = fonts.shape_line(text, FontFamily::Proportional, pixel_size);
            let galley = fonts.layout(
                text,
                FontId::proportional(size),
                crate::font::TextLayout::DEFAULT,
                1.0,
            );
            let mut pen = 0.0;

            for (shaped, placed) in shaped.iter().zip(galley.glyphs()) {
                let x =
                    placed.offset.x - placed.image.left as f32 + placed.id.subpixel as f32 * phase;
                assert!(
                    (x - pen - shaped.x_offset).abs() <= phase / 2.0 + 1.0 / 1024.0,
                    "{text:?} at {size} put a letter at {x} rather than {pen}"
                );
                pen += shaped.x_advance;
            }
        }
    }
}
