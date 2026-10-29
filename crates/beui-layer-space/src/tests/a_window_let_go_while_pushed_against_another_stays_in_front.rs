use super::*;
use crate::carry::{PUSH_LINES, TURN_SECONDS};
use crate::contact;

#[test]
fn a_window_let_go_while_pushed_against_another_stays_in_front() {
    let (mut space, now) = space();
    let popped = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    send(&mut space, now, [scroll(PUSH_LINES - 0.2)]);
    assert!(space.mouse.carry.as_ref().unwrap().ghost().is_some());
    let grab = centre();
    send(&mut space, now, [button(grab, false)]);
    assert_eq!(space.mouse.carry, None);
    assert!(space.outlines().is_empty(), "letting go drops the ghost");
    let panel = space.panel(popped).unwrap();
    assert!(!contact::touching(panel, main(&space), space.head.position));
    let at = panel.world(pos2(SMALL.x / 2.0, SMALL.y / 2.0));
    let ray = crate::head::Ray {
        origin: space.head.position,
        direction: (at - space.head.position).normalize(),
    };
    assert!(
        panel.hit(ray).unwrap().distance < main(&space).hit(ray).unwrap().distance,
        "it rests in front, where it was pushed against"
    );
}
