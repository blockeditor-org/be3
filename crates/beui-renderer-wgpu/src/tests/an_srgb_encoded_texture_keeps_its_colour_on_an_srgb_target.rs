use super::*;

#[test]
fn an_srgb_encoded_texture_keeps_its_colour_on_an_srgb_target() {
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |_| {});
    let grey = [128, 128, 128, 255];
    let picture = texture_of(&target, 1, &[grey]);
    paint_quad(
        &target,
        &picture,
        beui_draw::texture_quad::Quad {
            rect: [0.0, 0.0, 64.0, 64.0],
            srgb_encoded: true,
            ..Default::default()
        },
    );
    let pixel = target.read().pixel(32, 32);
    assert!(
        pixel[..3].iter().all(|channel| channel.abs_diff(128) <= 1),
        "the encoded value is decoded before the target encodes it again: {pixel:?}"
    );
}
