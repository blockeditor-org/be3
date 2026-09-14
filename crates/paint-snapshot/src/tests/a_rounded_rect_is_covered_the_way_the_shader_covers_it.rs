use super::*;
use crate::RoundedRect;

#[test]
fn a_rounded_rect_is_covered_the_way_the_shader_covers_it() {
    let snapshot = Snapshot::of(
        Frame {
            size: [8, 8],
            pixels_per_point: 1.0,
            background: [0, 0, 0, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 8.0, 8.0],
                content: Content::RoundedRect(RoundedRect {
                    rect: [1.0, 1.0, 7.0, 7.0],
                    corner_radius: 3.0,
                    stroke_width: 0.0,
                    color: [255, 255, 255, 255],
                }),
            }],
        },
        BTreeMap::new(),
    );

    let image = crate::render(&snapshot, 0).unwrap();

    assert_eq!(image.get_pixel(4, 4).0, [255, 255, 255, 255]);
    assert_eq!(image.get_pixel(1, 1).0, [0, 0, 0, 255]);
    assert_eq!(image.get_pixel(0, 4).0, [0, 0, 0, 255]);
}
