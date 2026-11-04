use super::*;
use beui_runner::renderer::Renderer as _;

#[test]
fn an_offscreen_surface_draws_every_frame_it_noted_once_it_is_looked_at() {
    let left = Rect::from_min_max(pos2(4.0, 4.0), pos2(28.0, 28.0));
    let right = Rect::from_min_max(pos2(36.0, 36.0), pos2(60.0, 60.0));
    let mut surface = pollster::block_on(crate::offscreen::OffscreenSurface::new(
        SIZE,
        SIZE,
        Default::default(),
    ))
    .expect("no graphics adapter is available");
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let frame = |left_color: Color32| {
        context.run(|context| {
            let painter = context.painter();
            painter.rect_filled(left, 0.0, left_color);
            painter.rect_filled(right, 0.0, Color32::WHITE);
        })
    };

    surface.note(frame(Color32::WHITE).main(), 1.0, Color32::BLACK);
    assert!(
        !surface.present(Color32::BLACK),
        "presenting with nothing prepared does nothing"
    );
    surface.note(
        frame(Color32::from_rgb(255, 0, 0)).main(),
        1.0,
        Color32::BLACK,
    );
    surface.present(Color32::BLACK);
    let looked_at = frame(Color32::from_rgb(255, 0, 0));
    assert!(
        surface.prepare(looked_at.main(), 1.0, Color32::BLACK),
        "the frames noted before are still waiting to be drawn"
    );
    surface.present(Color32::BLACK);
    let capture = surface.capture().expect("an offscreen surface can be read");

    let pixel = |x: u32, y: u32| {
        let offset = ((y * capture.width + x) * 4) as usize;
        &capture.rgba[offset..offset + 3]
    };
    assert_eq!(
        pixel(48, 48),
        [255, 255, 255],
        "what only the first, unseen frame painted is drawn"
    );
    assert_eq!(
        pixel(16, 16),
        [255, 0, 0],
        "the latest frame's colours are drawn"
    );
    assert_eq!(pixel(32, 2), [0, 0, 0], "the background is cleared");
}
