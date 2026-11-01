use super::*;

#[test]
fn a_translucent_texture_quad_is_blended_once_over_what_is_behind_it() {
    let mut target = Target::in_format(wgpu::TextureFormat::Rgba8Unorm);
    target.draw(Color32::BLACK, Repaint::Everything, |_| {});
    let picture = texture_of(&target, 1, &[[255; 4]]);
    paint_quad(
        &target,
        &picture,
        beui_draw::texture_quad::Quad {
            rect: [0.0, 0.0, 64.0, 64.0],
            opacity: 0.5,
            ..Default::default()
        },
    );
    let red = target.read().pixel(32, 32)[0];
    assert!(
        red.abs_diff(128) <= 1,
        "half-covering white over black should be mid grey, not {red}"
    );
}
