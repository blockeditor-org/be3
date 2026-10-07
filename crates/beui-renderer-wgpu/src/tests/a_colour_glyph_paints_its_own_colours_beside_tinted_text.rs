use super::*;
use beui_core::font::{
    FontBackend, FontFamily, Galley, GalleyLine, Glyph, GlyphId, GlyphImage, Shaping, Wraps,
};

const SIDE: u32 = 16;

struct Swatches;

impl FontBackend for Swatches {
    fn build(
        &mut self,
        text: &str,
        _family: FontFamily,
        pixel_size: u32,
        _shape: Shaping,
        _pixels_per_point: f32,
    ) -> Galley {
        let colours = (0..SIDE * SIDE)
            .flat_map(|at| match at < SIDE * SIDE / 2 {
                true => [255, 0, 0, 255],
                false => [0, 0, 255, 255],
            })
            .collect();
        let glyph = |glyph: u32, x: f32, pixels: Vec<u8>, color: bool| Glyph {
            id: GlyphId {
                face: 0,
                glyph,
                pixel_size,
                subpixel: 0,
                bold: false,
                italic: false,
            },
            image: Rc::new(GlyphImage {
                width: SIDE,
                height: SIDE,
                left: 0,
                top: 0,
                pixels,
                color,
            }),
            offset: vec2(x, 8.0),
        };
        Galley::new(
            text,
            FontId::proportional(pixel_size as f32),
            vec2(48.0, 32.0),
            32.0,
            24.0,
            vec![
                glyph(1, 8.0, colours, true),
                glyph(2, 32.0, vec![255; (SIDE * SIDE) as usize], false),
            ],
            vec![GalleyLine {
                top: 0.0,
                range: 0..text.len(),
                cursors: vec![(0, 0.0), (text.len(), 48.0)],
            }],
            Wraps::ANY,
        )
    }
}

#[test]
fn a_colour_glyph_paints_its_own_colours_beside_tinted_text() {
    let mut target = Target::new();
    target.draw_in(
        &Context::new(Swatches),
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| {
            painter.text(
                Pos2::ZERO,
                "ab",
                FontId::proportional(16.0),
                Color32::from_rgb(0, 255, 0),
            );
        },
    );
    let capture = target.read();

    let near = |pixel: [u8; 4], expected: [u8; 3]| {
        pixel
            .iter()
            .zip(expected)
            .all(|(got, want)| got.abs_diff(want) <= 2)
    };
    let top = capture.pixel(12, 10);
    let bottom = capture.pixel(12, 21);
    let tinted = capture.pixel(36, 14);
    assert!(
        near(top, [255, 0, 0]),
        "the top of the colour glyph keeps its red rather than taking the text colour, got {top:?}"
    );
    assert!(
        near(bottom, [0, 0, 255]),
        "the bottom of the colour glyph keeps its blue, got {bottom:?}"
    );
    assert!(
        near(tinted, [0, 255, 0]),
        "a plain glyph in the same run still takes the text colour, got {tinted:?}"
    );
}
