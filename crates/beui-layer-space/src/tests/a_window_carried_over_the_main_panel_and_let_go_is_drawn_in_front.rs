use beui_core::app::{Setup, Waker};
use beui_core::context::Context;
use beui_font_freetype::FreetypeFonts;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_window_carried_over_the_main_panel_and_let_go_is_drawn_in_front() {
    let (mut space, now) = space();
    let context = Context::new(FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    let popped = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    let to = centre() + vec2(120.0, 60.0);
    send(&mut space, now, [Input::PointerMoved(to)]);
    send(&mut space, now, [button(to, false)]);
    assert_eq!(space.mouse.carry, None);

    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, popped, SMALL, "Popped out"),
    ];
    let frame = space.scene(&windows, &view(now));
    let painting = crate::picture::painting(&frame, Rect::from_min_size(Pos2::ZERO, PLATFORM))
        .expect("the scene paints");
    let snapshot = paint_snapshot::Snapshot::of_painting(painting).expect("the painting encodes");
    paint_snapshot::assert_accepted(
        "a_window_carried_over_the_main_panel_and_let_go_is_drawn_in_front",
        &snapshot,
        "./scripts/test a_window_carried_over_the_main_panel_and_let_go_is_drawn_in_front --update",
    );
}
