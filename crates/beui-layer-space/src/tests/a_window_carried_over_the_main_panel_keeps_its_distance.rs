use beui_core::app::{Setup, Waker};
use beui_core::context::Context;
use beui_font_freetype::FreetypeFonts;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_window_carried_over_the_main_panel_keeps_its_distance() {
    let (mut space, now) = space();
    let context = Context::new(FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    let popped = carry_popped_out(&mut space, now);
    let distance = |space: &Space| {
        let carry = space.mouse.carry.as_ref().expect("it is carried");
        carry.grabbed_at(&space.head).distance(space.head.position)
    };
    let picked_up = distance(&space);
    let now = later(now, TURN_SECONDS);
    let to = centre() + vec2(120.0, 60.0);
    send(&mut space, now, [Input::PointerMoved(to)]);
    let over = distance(&space);
    assert!(
        (over - picked_up).abs() < 1e-4,
        "carried over the main panel it keeps the distance it was picked up at: {picked_up} {over}"
    );
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
        "a_window_carried_over_the_main_panel_keeps_its_distance",
        &snapshot,
        "./scripts/test a_window_carried_over_the_main_panel_keeps_its_distance --update",
    );
}
