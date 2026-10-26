use beui_core::app::{Setup, Waker};

use super::*;

#[test]
fn switching_the_cursor_icon_keeps_the_cursor_renderer() {
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        None,
    ))
    .expect("no graphics adapter is available");
    let (mut space, now) = space();
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(gpu.setup());
    space.setup(&setup);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let mut draw = |icon: CursorIcon, pixels_per_point: f32| {
        let view = HostView {
            pixels_per_point,
            ..view(now)
        };
        space.frame(&app_frame(icon), &view);
        space.cursor_drawn().expect("the cursor is drawn")
    };

    let (renderer, size) = draw(CursorIcon::Text, 1.0);
    for icon in [
        CursorIcon::PointingHand,
        CursorIcon::Default,
        CursorIcon::Text,
        CursorIcon::Grab,
    ] {
        assert_eq!(
            draw(icon, 1.0),
            (renderer, size),
            "{icon:?} is painted into the texture the cursor had, by the renderer it had, \
             rather than building its pipelines and glyph atlas again for every new icon"
        );
    }
    let (again, larger) = draw(CursorIcon::Default, 2.0);
    assert_eq!(again, renderer, "a new scale keeps the renderer too");
    assert_eq!(
        larger,
        (size.0 * 2, size.1 * 2),
        "and draws at the new scale"
    );
}
