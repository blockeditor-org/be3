use super::*;

#[test]
fn a_group_paints_its_shapes_as_one_at_its_opacity() {
    let square = |rect: [f32; 4], color: [u8; 4]| Primitive {
        clip: [0.0, 0.0, 8.0, 8.0],
        fade: Fade::NONE,
        rounds: Vec::new(),
        content: Content::RoundedRect(RoundedRect {
            rect,
            corner_radius: 0.0,
            stroke_width: 0.0,
            color,
            turn: Turn::NONE,
        }),
    };
    let group = Group::primitive(
        0.5,
        vec![
            square([0.0, 0.0, 6.0, 6.0], [255, 255, 255, 255]),
            square([2.0, 2.0, 8.0, 8.0], [255, 0, 0, 255]),
        ],
    )
    .expect("the squares show");
    let frame = Frame {
        size: [8, 8],
        pixels_per_point: 1.0,
        background: [0, 0, 0, 255],
        primitives: vec![group],
    };
    let pixels = rasterize(&frame, [0, 0, 0, 255], |_| None).unwrap();
    let at = |x: usize, y: usize| pixels.rgba[y * 8 + x];

    assert_eq!(at(4, 4), at(7, 7), "the white square does not show through");
    let [red, green, blue, alpha] = at(4, 4);
    assert!(red.abs_diff(128) <= 1 && green == 0 && blue == 0 && alpha == 255);
    let [red, green, blue, _] = at(1, 1);
    assert!(red.abs_diff(128) <= 1 && red == green && green == blue);
}
