use super::*;
use crate::{RoundedRect, Turn};

#[test]
fn a_turned_rounded_rect_is_covered_where_it_turned_to() {
    let bar = RoundedRect {
        rect: [2.0, 7.0, 14.0, 9.0],
        corner_radius: 0.0,
        stroke_width: 0.0,
        color: [255, 255, 255, 255],
        turn: Turn::NONE,
    };
    let upright = painting(bar);
    let turned = painting(RoundedRect {
        turn: Turn {
            pivot: [8.0, 8.0],
            angle: std::f32::consts::FRAC_PI_2,
        },
        ..bar
    });

    assert_eq!(upright.get_pixel(3, 8).0, [255, 255, 255, 255]);
    assert_eq!(upright.get_pixel(8, 3).0, [0, 0, 0, 255]);
    assert_eq!(turned.get_pixel(8, 3).0, [255, 255, 255, 255]);
    assert_eq!(turned.get_pixel(3, 8).0, [0, 0, 0, 255]);
}

fn painting(shape: RoundedRect) -> image::RgbaImage {
    let snapshot = Snapshot::of(
        Frame {
            size: [16, 16],
            pixels_per_point: 1.0,
            background: [0, 0, 0, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 16.0, 16.0],
                content: Content::RoundedRect(shape),
            }],
        },
        BTreeMap::new(),
    );
    crate::render(&snapshot, 0).unwrap()
}
