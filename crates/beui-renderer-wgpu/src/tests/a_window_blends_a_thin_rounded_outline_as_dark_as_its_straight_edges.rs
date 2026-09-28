use super::*;
use crate::present::surface_format;

const RADIUS: f32 = 8.0;

#[test]
fn a_window_blends_a_thin_rounded_outline_as_dark_as_its_straight_edges() {
    let format = surface_format(&[
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Rgba8Unorm,
    ])
    .expect("a format is offered");
    let capture = capture_in(format, Color32::WHITE, |painter| {
        painter.rect_stroke(
            Rect::from_min_max(pos2(8.0, 8.0), pos2(56.0, 56.0)),
            RADIUS,
            1.0,
            Color32::BLACK,
        );
    });
    let ink = |left: u32, top: u32, right: u32, bottom: u32| -> f32 {
        let mut ink = 0.0;
        for y in top..bottom {
            for x in left..right {
                ink += 1.0 - f32::from(capture.pixel(x, y)[0]) / 255.0;
            }
        }
        ink
    };

    let edge = ink(8, 24, 9, 40) / 16.0;
    let arc = std::f32::consts::FRAC_PI_2 * (RADIUS - 0.5);
    let corner = ink(8, 8, 16, 16) / arc;
    assert!(edge > 0.95, "the straight edge is solid, {edge}");
    assert!(
        corner > edge * 0.9,
        "the rounded corner is lighter than the edge: {corner} against {edge}"
    );
}
