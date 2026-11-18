use super::*;
use be_lib_layers::{LayerId, Surface};
use beui_draw::compose::{layer_of, quad};
use beui_draw::texture_quad::Quad;

#[test]
fn a_layer_of_a_turned_and_clipped_quad_shows_what_the_quad_shows() {
    let close = |one: [f32; 4], other: [f32; 4]| {
        one.iter()
            .zip(other)
            .all(|(one, other)| (one - other).abs() < 1e-3)
    };
    // A program window of 100 by 50 pixels drawn at two pixels a point, its right half cut
    // off by its pane, its corners rounded.
    let clipped = Quad {
        rect: [20.0, 40.0, 220.0, 140.0],
        clip: [0.0, 0.0, 120.0, 1000.0],
        corner_radii: [8.0; 4],
        ..Quad::default()
    };
    let layer =
        layer_of(&clipped, (100, 50), 2.0, LayerId(1), Surface::Host(1)).expect("the window shows");
    assert_eq!(
        layer.clip,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(50.0, 50.0)),
        "only the half its pane lets through is shown"
    );
    assert_eq!(layer.corner_radius, 4.0, "its rounding, in points");
    assert_eq!(layer.rounded(), [true, false, false, true]);
    let drawn = quad(&layer, 2.0, [0.0; 2]);
    assert!(
        close(drawn.rect, [20.0, 40.0, 120.0, 140.0]) && close(drawn.uv, [0.0, 0.0, 0.5, 1.0]),
        "and drawn where the quad drew it: {drawn:?}"
    );

    let turned = Quad {
        rect: [20.0, 40.0, 220.0, 140.0],
        rotation: std::f32::consts::FRAC_PI_2,
        ..Quad::default()
    };
    let layer =
        layer_of(&turned, (100, 50), 2.0, LayerId(1), Surface::Host(1)).expect("the window shows");
    let drawn = quad(&layer, 2.0, [0.0; 2]);
    assert!(
        close(drawn.rect, turned.rect)
            && close(drawn.uv, [0.0, 0.0, 1.0, 1.0])
            && (drawn.rotation - turned.rotation).abs() < 1e-4,
        "a turned window is drawn turned where the quad drew it: {drawn:?}"
    );
}
