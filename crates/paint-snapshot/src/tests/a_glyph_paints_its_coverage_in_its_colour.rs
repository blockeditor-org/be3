use super::*;
use crate::Glyph;

#[test]
fn a_glyph_paints_its_coverage_in_its_colour() {
    let coverage = Texture::encode([2, 1], &[[255; 4], [0; 4]]).unwrap();
    let snapshot = Snapshot::of(
        Frame {
            size: [4, 4],
            pixels_per_point: 1.0,
            background: [0, 0, 0, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 4.0, 4.0],
                content: Content::Glyph(Glyph {
                    rect: [1.0, 1.0, 3.0, 2.0],
                    texture: 7,
                    color: [0, 255, 0, 255],
                }),
            }],
        },
        BTreeMap::from([(7, coverage)]),
    );

    let image = crate::render(&snapshot, 0).unwrap();

    assert_eq!(image.get_pixel(1, 1).0, [0, 255, 0, 255]);
    assert_eq!(image.get_pixel(2, 1).0, [0, 0, 0, 255]);
    assert_eq!(image.get_pixel(1, 2).0, [0, 0, 0, 255]);
}
