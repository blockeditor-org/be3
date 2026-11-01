use beui_core::app::{Setup, Waker};
use beui_core::context::Context;
use beui_font_freetype::FreetypeFonts;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_panel_carried_to_each_corner_with_the_head_rolled_and_pitched_stays_upright_to_the_view() {
    let (mut space, now) = space();
    let context = Context::new(FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    roll_and_look_down(&mut space);
    space.panels[0].metres_per_point *= 0.4;
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );
    let grabbed = main(&space)
        .hit(space.head.ray(PLATFORM, centre().to_vec2()))
        .unwrap()
        .pos;
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    let corners = [
        pos2(120.0, 120.0),
        pos2(PLATFORM.x - 120.0, 120.0),
        pos2(PLATFORM.x - 120.0, PLATFORM.y - 120.0),
        pos2(120.0, PLATFORM.y - 120.0),
    ];
    let mut at = now;
    for corner in corners {
        send(&mut space, at, [Input::PointerMoved(corner)]);
        at = later(at, TURN_SECONDS);
        send(&mut space, at, []);
        let panel = main(&space);
        assert!(
            facing_eye(&space, panel) < 1e-3,
            "at {corner:?} it faces the eye"
        );
        assert!(
            upright_to(&space, panel) < 1e-3,
            "at {corner:?} its up is the head's up: {}",
            upright_to(&space, panel)
        );
        let under = panel
            .hit(space.head.ray(PLATFORM, corner.to_vec2()))
            .expect("the panel is under the cursor")
            .pos;
        assert!(
            (under - grabbed).length() < 0.5,
            "at {corner:?} the grabbed point stays under the cursor: {grabbed:?} then {under:?}"
        );
    }
    send(&mut space, at, [button(corners[3], false)]);
    assert_eq!(space.mouse.carry, None);
    assert!(
        upright_to(&space, main(&space)) < 1e-3,
        "and after letting go"
    );

    let windows = [window(&context, WindowId::MAIN, PLATFORM, "Rolled")];
    let frame = space.scene(&windows, &view(at));
    let painting = crate::picture::painting(&frame, Rect::from_min_size(Pos2::ZERO, PLATFORM))
        .expect("the scene paints");
    paint_snapshot::assert_painting(
        "a_panel_carried_to_each_corner_with_the_head_rolled_and_pitched_stays_upright_to_the_view",
        painting,
    );
}
