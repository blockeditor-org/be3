use beui_core::app::{Setup, Waker};
use beui_core::painting::Punches;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::SKY;
use crate::notice::Notice;

#[test]
fn the_mirror_shows_a_notice_over_the_scene() {
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        Default::default(),
    ))
    .expect("no graphics adapter is available");
    let (mut space, now) = space();
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(gpu.setup());
    setup.provide(context.clone());
    space.setup(&setup);
    let mut output = app_frame(CursorIcon::Default);
    output.windows = vec![window(&context, WindowId::MAIN, PLATFORM, "Main")];
    let bare = space.frame(&output, &view(now)).window.layers.len();

    space.set_notice(Some(Notice {
        title: "Waiting for a headset".to_owned(),
        detail: "Monado 21.0.0 finds no headset".to_owned(),
    }));
    let shown = space.frame(&output, &view(now)).window;
    assert!(
        shown.layers.len() > bare,
        "the notice is drawn over the scene"
    );
    let painting = beui_core::painting::painting(&shown, PLATFORM, 1.0, SKY, Punches::Shown)
        .expect("the mirror paints");
    let snapshot = paint_snapshot::Snapshot::of_painting(painting).expect("the painting encodes");
    paint_snapshot::assert_accepted(
        "the_mirror_shows_a_notice_over_the_scene",
        &snapshot,
        "./scripts/test the_mirror_shows_a_notice_over_the_scene --update",
    );

    space.set_notice(None);
    assert_eq!(
        space.frame(&output, &view(now)).window.layers.len(),
        bare,
        "and goes once there is no notice"
    );
}
