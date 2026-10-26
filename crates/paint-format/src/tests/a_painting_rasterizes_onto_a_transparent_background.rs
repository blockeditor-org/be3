use super::*;

#[test]
fn a_painting_rasterizes_onto_a_transparent_background() {
    let white = Pixels {
        size: [1, 1],
        rgba: vec![[255, 255, 255, 255]],
    };
    let corner = |x: f32, y: f32| Vertex {
        pos: [x, y],
        uv: [0.5, 0.5],
        color: [200, 100, 0, 128],
    };
    let frame = Frame {
        size: [8, 8],
        pixels_per_point: 2.0,
        background: [0, 0, 0, 0],
        primitives: vec![Primitive {
            clip: [0.0, 0.0, 4.0, 4.0],
            fade: Fade::NONE,
            content: Content::Mesh(vec![Triangle {
                texture: 0,
                corners: [corner(0.0, 0.0), corner(4.0, 0.0), corner(0.0, 4.0)],
            }]),
        }],
    };
    let pixels = rasterize(&frame, [0, 0, 0, 0], |_| Some(&white)).unwrap();
    assert_eq!(pixels.size, [8, 8]);
    let [red, green, blue, alpha] = pixels.rgba[8 + 1];
    assert_eq!(alpha, 128);
    assert!(red.abs_diff(200) <= 1 && green.abs_diff(100) <= 1 && blue == 0);
    assert_eq!(pixels.rgba[8 * 7 + 7], [0, 0, 0, 0]);
}
