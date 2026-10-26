use super::*;
use crate::render::Compositor;

#[test]
fn a_resized_panel_keeps_its_renderer_and_draws_at_the_new_size() {
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        Default::default(),
    ))
    .expect("no graphics adapter is available");
    let compositor = Compositor::new(&gpu.setup());
    let mut output = app_frame(CursorIcon::Default).main().clone();
    let mut draw = |size: Vec2| {
        output.size = size;
        output.changed = true;
        compositor.update(&output);
        compositor.paint_windows();
        compositor
            .drawn(WindowId::MAIN)
            .expect("the window's panel is drawn")
    };

    let (renderer, first) = draw(vec2(200.0, 150.0));
    assert_eq!(first, (200, 150));
    for step in 1..=3 {
        let size = vec2(200.0 + step as f32 * 30.0, 150.0 + step as f32 * 10.0);
        let (again, texture) = draw(size);
        assert_eq!(
            texture,
            (size.x as u32, size.y as u32),
            "drawn at its new size"
        );
        assert_eq!(
            again, renderer,
            "by the renderer it had, which keeps its pipelines and glyph atlases \
             rather than building them again for every step of a resize"
        );
    }
}
