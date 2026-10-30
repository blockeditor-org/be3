use super::*;
use crate::{RoundedRect, Turn};

#[test]
fn a_faded_rect_fades_toward_the_edge_it_names_the_way_the_shader_does() {
    let snapshot = Snapshot::of(
        Frame {
            size: [8, 2],
            pixels_per_point: 2.0,
            background: [0, 0, 0, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 4.0, 1.0],
                fade: Fade {
                    rect: [0.0, 0.0, 4.0, 1.0],
                    widths: [2.0, 0.0, 0.0, 0.0],
                    opacity: 1.0,
                },
                rounds: Vec::new(),
                content: Content::RoundedRect(RoundedRect {
                    rect: [0.0, 0.0, 4.0, 1.0],
                    corner_radius: 0.0,
                    stroke_width: 0.0,
                    color: [255, 255, 255, 255],
                    turn: Turn::NONE,
                }),
            }],
        },
        BTreeMap::new(),
    );

    let image = crate::render(&snapshot, 0).unwrap();

    assert_eq!(image.get_pixel(0, 0).0, [32, 32, 32, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [96, 96, 96, 255]);
    assert_eq!(image.get_pixel(4, 1).0, [255, 255, 255, 255]);
    assert_eq!(image.get_pixel(7, 1).0, [255, 255, 255, 255]);
}
