use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::{PUSH_LINES, Side, TURN_SECONDS};
use crate::contact;
use crate::head::Ray;

fn side(space: &Space) -> Option<Side> {
    space.mouse.carry.as_ref().unwrap().side(WindowId::MAIN)
}

fn ghosted(space: &Space) -> bool {
    space.mouse.carry.as_ref().unwrap().ghost().is_some()
}

fn clear(space: &Space, window: WindowId) -> bool {
    !contact::touching(
        space.panel(window).unwrap(),
        main(space),
        space.head.position,
    )
}

fn behind(space: &Space, window: WindowId) -> bool {
    let panel = space.panel(window).unwrap();
    let point = panel.world(pos2(panel.size.x / 2.0, panel.size.y / 2.0));
    let ray = Ray {
        origin: space.head.position,
        direction: (point - space.head.position).normalize(),
    };
    main(space).hit(ray).unwrap().distance < panel.hit(ray).unwrap().distance
}

#[test]
fn pushing_a_carried_window_through_another_takes_a_deliberate_push_and_keeps_it_behind() {
    let (mut space, now, context) = painted_space();
    let popped = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);

    send(&mut space, now, [scroll(1.0)]);
    assert_eq!(side(&space), Some(Side::Front));
    assert!(clear(&space, popped), "a line farther rests it against the main panel");
    send(&mut space, now, [scroll(1.0)]);
    assert!(ghosted(&space), "pushing on shows where it is being pushed");
    assert_eq!(side(&space), Some(Side::Front), "but it does not go through yet");
    assert!(clear(&space, popped) && !behind(&space, popped));
    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, popped, SMALL, "Popped out"),
    ];
    assert_scene(
        &mut space,
        now,
        &windows,
        "pushing_a_carried_window_through_another_shows_its_ghost",
    );

    send(&mut space, now, [scroll(PUSH_LINES)]);
    assert_eq!(side(&space), Some(Side::Behind), "a deliberate push goes through");
    assert!(!ghosted(&space));
    assert!(clear(&space, popped) && behind(&space, popped));

    for step in 1..=6 {
        let to = centre() + vec2(-45.0 * step as f32, 20.0 * step as f32);
        send(&mut space, now, [Input::PointerMoved(to)]);
        assert_eq!(side(&space), Some(Side::Behind), "step {step}");
        assert!(clear(&space, popped), "dragged behind it never clips, step {step}");
        assert!(behind(&space, popped), "step {step}");
    }

    send(&mut space, now, [scroll(-1.0)]);
    assert_eq!(side(&space), Some(Side::Behind));
    send(&mut space, now, [scroll(-1.0)]);
    assert!(ghosted(&space), "pulling it nearer pushes against the main panel from behind");
    assert_eq!(side(&space), Some(Side::Behind));
    send(&mut space, now, [scroll(-PUSH_LINES)]);
    assert_eq!(side(&space), Some(Side::Front), "and a deliberate pull brings it back in front");
    assert!(clear(&space, popped) && !behind(&space, popped));
}
