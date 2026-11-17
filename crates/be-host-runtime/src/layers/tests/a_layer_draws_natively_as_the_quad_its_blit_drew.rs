use super::*;

#[test]
fn a_layer_draws_natively_as_the_quad_its_blit_drew() {
    let quads = [
        Quad::upright(Rect::from_min_size(pos2(10.0, 40.0), vec2(150.0, 100.0))),
        Quad {
            rect: Rect::from_min_size(pos2(30.0, 50.0), vec2(120.0, 80.0)),
            rotation: 0.4,
            opacity: 0.5,
            corner_radius: 6.0,
        },
    ];
    let source = Rect::from_min_max(pos2(0.25, 0.0), pos2(1.0, 0.5));
    for quad in quads {
        let layer = screen(
            LayerId(1),
            Surface::Root(0),
            (quad, source),
            Painted {
                size: (300, 200),
                raster: 2.0,
            },
        )
        .expect("the blit shows");
        let drawn = texture_quad(&layer, &at());
        let was = quad.placed(source, &at());
        assert!(
            close(drawn.rect, was.rect)
                && close(drawn.uv, was.uv)
                && drawn.clip == was.clip
                && (drawn.rotation - was.rotation).abs() < 1e-5
                && (drawn.opacity, drawn.corner_radius) == (was.opacity, was.corner_radius),
            "{drawn:?} draws as {was:?} did"
        );
    }
}
