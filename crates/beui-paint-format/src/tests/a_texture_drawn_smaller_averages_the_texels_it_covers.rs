use super::*;

#[test]
fn a_texture_drawn_smaller_averages_the_texels_it_covers() {
    const SIDE: u32 = 64;
    let checks = Pixels {
        size: [SIDE, SIDE],
        rgba: (0..SIDE * SIDE)
            .map(|at| match (at % SIDE + at / SIDE) % 2 {
                0 => [255, 255, 255, 255],
                _ => [0, 0, 0, 255],
            })
            .collect(),
    };
    let corner = |x: f32, y: f32| Vertex {
        pos: [x * 8.0, y * 8.0],
        uv: [x, y],
        color: [255, 255, 255, 255],
    };
    let frame = Frame {
        size: [8, 8],
        pixels_per_point: 1.0,
        background: [0, 0, 0, 255],
        primitives: vec![Primitive {
            clip: [0.0, 0.0, 8.0, 8.0],
            fade: Fade::NONE,
            rounds: Vec::new(),
            content: Content::Mesh(vec![
                Triangle {
                    texture: 1,
                    corners: [corner(0.0, 0.0), corner(1.0, 0.0), corner(0.0, 1.0)],
                },
                Triangle {
                    texture: 1,
                    corners: [corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)],
                },
            ]),
        }],
    };
    let pixels = rasterize(&frame, [0, 0, 0, 255], |_| Some(&checks)).unwrap();
    for pixel in &pixels.rgba {
        assert!(
            pixel[0].abs_diff(128) <= 2,
            "a checkerboard eight times too fine to show draws as its grey, not as a few of its texels: {pixel:?}"
        );
    }
}
