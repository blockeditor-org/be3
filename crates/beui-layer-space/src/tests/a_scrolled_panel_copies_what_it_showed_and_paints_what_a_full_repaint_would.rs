use super::*;
use crate::render::Compositor;
use beui_core::context::{Moved, WindowOutput};
use beui_types::Color32;
use beui_types::geometry::Region;

const SIZE: Vec2 = vec2(64.0, 80.0);

fn rows(offset: f32) -> WindowOutput {
    let fonts = beui_font_freetype::FreetypeFonts::new(beui_font_freetype::FontLibrary::new(
        beui_font_freetype::FontSources::default(),
    ));
    let mut output = Context::new(fonts)
        .run(|context| {
            let painter = context.painter();
            for row in 0..12 {
                let top = row as f32 * 10.0 + offset;
                let shade = (row * 20) as u8;
                painter.rect_filled(
                    Rect::from_min_size(pos2(4.0, top + 2.0), vec2(40.0 + row as f32, 6.0)),
                    0.0,
                    Color32::from_rgb(shade, 255 - shade, 90),
                );
            }
        })
        .main()
        .clone();
    output.size = SIZE;
    output.changed = true;
    output
}

#[test]
fn a_scrolled_panel_copies_what_it_showed_and_paints_what_a_full_repaint_would() {
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        Default::default(),
    ))
    .expect("no graphics adapter is available");
    let draw = |scrolled: &WindowOutput| {
        let compositor = Compositor::new(&gpu.setup());
        compositor.update(&rows(0.0));
        compositor.paint_windows();
        compositor.update(scrolled);
        compositor.paint_windows();
        compositor
            .read(WindowId::MAIN)
            .expect("the panel reads back")
    };

    let mut scrolled = rows(-10.0);
    scrolled.moved = Some(Moved {
        from: Rect::from_min_max(pos2(0.0, 10.0), pos2(SIZE.x, SIZE.y)),
        by: vec2(0.0, -10.0),
    });
    scrolled.damage = Region::from(Rect::from_min_max(
        pos2(0.0, SIZE.y - 10.0),
        pos2(SIZE.x, SIZE.y),
    ));
    let copied = draw(&scrolled);
    let unmoved = draw(&WindowOutput {
        moved: None,
        ..scrolled.clone()
    });

    let whole = Compositor::new(&gpu.setup());
    whole.update(&rows(-10.0));
    whole.paint_windows();

    let whole = whole.read(WindowId::MAIN).expect("the panel reads back");
    assert!(
        unmoved.rgba != whole.rgba,
        "repainting only the exposed strip without the copy leaves the old rows"
    );
    assert!(
        copied.rgba == whole.rgba,
        "the rows the copy moved and the strip it repainted are what painting it all draws"
    );
}
