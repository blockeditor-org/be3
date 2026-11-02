use super::*;

#[test]
fn a_renderer_prepared_before_the_atlas_is_renewed_still_paints_its_glyphs() {
    let expected = capture(Color32::BLACK, |painter| {
        painter.text(
            pos2(4.0, 4.0),
            "beui",
            FontId::proportional(24.0),
            Color32::WHITE,
        );
    });
    let mut first = Target::new();
    let mut second = Target::sharing(&first);
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let output = context.run(|context| {
        context.painter().text(
            pos2(4.0, 4.0),
            "beui",
            FontId::proportional(24.0),
            Color32::WHITE,
        );
    });
    first.renderer.prepare(
        &first.device,
        &first.queue,
        output.main(),
        vec2(SIZE as f32, SIZE as f32),
        1.0,
        Repaint::Everything,
    );

    let resources = first.renderer.resources().clone();
    resources.0.borrow_mut().renew();
    second.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.text(
            pos2(4.0, 4.0),
            "WXYZ",
            FontId::proportional(24.0),
            Color32::WHITE,
        );
    });

    let mut encoder = first
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    first.renderer.render(
        &first.device,
        &first.queue,
        &mut encoder,
        &first.view,
        (SIZE, SIZE),
        wgpu::LoadOp::Clear(wgpu_color_in(first.format, Color32::BLACK)),
    );
    first.queue.submit(Some(encoder.finish()));

    assert_eq!(
        first.read().pixels,
        expected.pixels,
        "glyphs encoded before the atlas was renewed still read the atlas they were placed in"
    );
}
