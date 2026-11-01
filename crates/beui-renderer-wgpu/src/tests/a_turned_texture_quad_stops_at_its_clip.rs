use super::*;

#[test]
fn a_turned_texture_quad_stops_at_its_clip() {
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |_| {});
    let (red, blue) = ([255, 0, 0, 255], [0, 0, 255, 255]);
    let picture = texture_of(&target, 2, &[red, blue]);
    paint_quad(
        &target,
        &picture,
        beui_draw::texture_quad::Quad {
            rect: [16.0, 24.0, 48.0, 40.0],
            clip: [0.0, 0.0, 64.0, 44.0],
            rotation: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        },
    );
    let capture = target.read();
    assert_eq!(
        capture.pixel(32, 18),
        red,
        "a quarter turn clockwise brings the picture's left edge to the top"
    );
    assert_eq!(
        capture.pixel(32, 42),
        blue,
        "and its right edge to the bottom"
    );
    assert_eq!(
        capture.pixel(32, 46),
        [0, 0, 0, 255],
        "nothing is drawn past the clip"
    );
    assert_eq!(
        capture.pixel(20, 32),
        [0, 0, 0, 255],
        "the turned quad is as wide as the rect was tall"
    );
}
