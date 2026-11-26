use super::*;
use beui_paint_snapshot::{RoundedRect, Turn};

#[test]
fn a_rounded_rect_covers_its_corners_by_the_area_inside_them() {
    let snapshot = Snapshot::of(
        Frame {
            size: [8, 8],
            pixels_per_point: 1.0,
            background: [0, 0, 0, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 8.0, 8.0],
                fade: Fade::NONE,
                rounds: Vec::new(),
                content: Content::RoundedRect(RoundedRect {
                    rect: [1.0, 1.0, 7.0, 7.0],
                    corner_radius: 3.0,
                    stroke_width: 0.0,
                    color: [255, 255, 255, 255],
                    turn: Turn::NONE,
                }),
            }],
        },
        BTreeMap::new(),
    );

    let image = crate::render(&snapshot, 0).unwrap();

    assert_eq!(image.get_pixel(4, 4).0, [255, 255, 255, 255]);
    // The corner pixel holds only a sliver of the rounding.
    assert_eq!(image.get_pixel(1, 1).0, [1, 1, 1, 255]);
    assert_eq!(image.get_pixel(0, 4).0, [0, 0, 0, 255]);
}
