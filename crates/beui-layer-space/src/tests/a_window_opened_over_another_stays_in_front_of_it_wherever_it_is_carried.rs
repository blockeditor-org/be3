use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::{Side, TURN_SECONDS};
use crate::contact;
use crate::head::Ray;

fn nearer(space: &Space, near: WindowId, far: WindowId) -> bool {
    let panel = space.panel(near).unwrap();
    let point = panel.world(pos2(panel.size.x / 2.0, panel.size.y / 2.0));
    let ray = Ray {
        origin: space.head.position,
        direction: (point - space.head.position).normalize(),
    };
    let (Some(a), Some(b)) = (panel.hit(ray), space.panel(far).unwrap().hit(ray)) else {
        panic!("both windows are under the middle of {near:?}");
    };
    a.distance < b.distance
}

#[test]
fn a_window_opened_over_another_stays_in_front_of_it_wherever_it_is_carried() {
    let (mut space, now, context) = painted_space();
    let popped = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    for step in 1..=8 {
        let to = centre() + vec2(40.0 * step as f32, 18.0 * step as f32);
        send(&mut space, now, [Input::PointerMoved(to)]);
        let carried = space.panel(popped).unwrap();
        assert!(
            !contact::touching(carried, main(&space), space.head.position),
            "it never sinks into the main panel, step {step}"
        );
        assert_eq!(
            space.mouse.carry.as_ref().unwrap().side(WindowId::MAIN),
            Some(Side::Front)
        );
        assert!(nearer(&space, popped, WindowId::MAIN), "step {step}");
    }
    let to = centre() + vec2(320.0, 144.0);
    send(&mut space, now, [button(to, false)]);
    assert_eq!(space.mouse.carry, None);
    assert!(nearer(&space, popped, WindowId::MAIN), "and stays there");

    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, popped, SMALL, "Popped out"),
    ];
    assert_scene(
        &mut space,
        now,
        &windows,
        "a_window_opened_over_another_stays_in_front_of_it_wherever_it_is_carried",
    );
}
