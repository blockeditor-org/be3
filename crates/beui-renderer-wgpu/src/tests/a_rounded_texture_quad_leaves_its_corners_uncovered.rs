use super::*;

#[test]
fn a_rounded_texture_quad_leaves_its_corners_uncovered() {
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |_| {});
    let white = [255; 4];
    let picture = texture_of(&target, 2, &[white; 4]);
    paint_quad(
        &target,
        &picture,
        beui_draw::texture_quad::Quad {
            rect: [0.0, 0.0, 64.0, 64.0],
            corner_radius: 16.0,
            ..Default::default()
        },
    );
    let capture = target.read();
    let black = [0, 0, 0, 255];
    assert_eq!(
        capture.pixel(0, 0),
        black,
        "the corner outside the rounding stays black"
    );
    assert_eq!(capture.pixel(63, 63), black, "every corner is rounded");
    assert_eq!(
        capture.pixel(32, 0),
        white,
        "the edge between the corners is covered"
    );
    assert_eq!(capture.pixel(8, 8), white, "inside the rounding is covered");

    target.draw(Color32::BLACK, Repaint::Everything, |_| {});
    paint_quad(
        &target,
        &picture,
        beui_draw::texture_quad::Quad {
            rect: [0.0, 0.0, 32.0, 64.0],
            uv: [0.0, 0.0, 0.5, 1.0],
            corner_radius: 16.0,
            ..Default::default()
        },
    );
    let capture = target.read();
    assert_eq!(
        capture.pixel(0, 0),
        black,
        "a corner of the whole texture is rounded"
    );
    assert_eq!(
        capture.pixel(31, 0),
        white,
        "where the source rect cuts the texture, its edge stays square"
    );
}
