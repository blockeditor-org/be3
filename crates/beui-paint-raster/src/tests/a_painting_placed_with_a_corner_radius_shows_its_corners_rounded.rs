use super::*;

#[test]
fn a_painting_placed_with_a_corner_radius_shows_its_corners_rounded() {
    let square = Painting {
        frame: Frame {
            size: [20, 20],
            pixels_per_point: 1.0,
            background: [0, 0, 0, 0],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 20.0, 20.0],
                fade: Fade::NONE,
                rounds: Vec::new(),
                content: Content::RoundedRect(RoundedRect {
                    rect: [0.0, 0.0, 20.0, 20.0],
                    corner_radius: 0.0,
                    stroke_width: 0.0,
                    color: [255, 255, 255, 255],
                    turn: Turn::NONE,
                }),
            }],
        },
        textures: BTreeMap::new(),
    };
    let primitives = square.placed(Placement {
        source: [0.0, 0.0, 1.0, 1.0],
        corners: [[10.0, 10.0], [50.0, 10.0], [50.0, 50.0], [10.0, 50.0]],
        clip: [0.0, 0.0, 60.0, 60.0],
        corner_radius: 8.0,
    });
    let frame = Frame {
        size: [60, 60],
        pixels_per_point: 1.0,
        background: [0, 0, 0, 0],
        primitives,
    };
    let pixels = rasterize(&frame, [0, 0, 0, 0], |_| None).unwrap();
    let alpha = |x: u32, y: u32| pixels.rgba[(y * 60 + x) as usize][3];
    assert_eq!(alpha(10, 10), 0, "the placed square's corner is cut away");
    assert_eq!(alpha(49, 49), 0, "and so is every other corner");
    assert_eq!(alpha(30, 10), 255, "the edge between the corners is kept");
    assert_eq!(alpha(14, 14), 255, "inside the rounding is kept");
}
