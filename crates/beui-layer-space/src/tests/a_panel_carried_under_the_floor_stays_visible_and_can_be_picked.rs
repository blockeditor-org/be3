use beui_core::app::{Setup, Waker};
use beui_font_freetype::FreetypeFonts;

use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::TURN_SECONDS;

const SIZE: Vec2 = vec2(800.0, 500.0);

#[test]
fn a_panel_carried_under_the_floor_stays_visible_and_can_be_picked() {
    let (mut space, now) = space();
    let context = Context::new(FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    space.head.yaw = std::f32::consts::PI;
    space.head.pitch = -0.6;
    let head = space.head;
    let under = WindowId::next();
    let pose = Pose {
        position: head.position + head.rotation() * Vec3::new(0.0, 0.0, -1.5),
        rotation: head.rotation(),
    };
    space.request(&WindowRequest::Open {
        window: under,
        placement: WindowPlacement::Host(panel(under, pose).placement()),
        size: SIZE,
        role: WindowRole::Normal,
    });
    send(&mut space, now, []);

    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );
    space.request(&WindowRequest::BeginMove(under));
    send(&mut space, now, []);
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [scroll(22.0)]);
    let carried = space.panel(under).expect("the window has a panel");
    let highest = [pos2(0.0, 0.0), pos2(SIZE.x, 0.0)]
        .map(|corner| carried.world(corner).y)
        .into_iter()
        .fold(f32::MIN, f32::max);
    assert!(
        highest < -0.3,
        "the carried panel is wholly under the floor, its top at {highest}"
    );
    let ray = space.cursor_ray().expect("the cursor has a ray");
    assert_eq!(
        space.pick(ray).map(|hit| hit.window),
        Some(under),
        "the panel is picked under the floor while carried"
    );
    send(&mut space, now, [button(centre(), false)]);
    assert_eq!(space.mouse.carry, None);

    let got = delivered(&send(
        &mut space,
        now,
        [button(centre(), true), button(centre(), false)],
    ));
    assert!(
        !got.is_empty() && got.iter().all(|(to, _)| *to == under),
        "a click under the floor reaches the panel there: {got:?}"
    );

    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, under, SIZE, "Under the floor"),
    ];
    let frame = space.scene(&windows, &view(now));
    let painting = crate::picture::painting(&frame, Rect::from_min_size(Pos2::ZERO, PLATFORM))
        .expect("the scene paints");
    let snapshot = paint_snapshot::Snapshot::of_painting(painting).expect("the painting encodes");
    paint_snapshot::assert_accepted(
        "a_panel_carried_under_the_floor_stays_visible_and_can_be_picked",
        &snapshot,
        "./scripts/test a_panel_carried_under_the_floor_stays_visible_and_can_be_picked --update",
    );
}
