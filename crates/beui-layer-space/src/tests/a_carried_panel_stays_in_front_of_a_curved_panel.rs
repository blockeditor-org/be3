use super::*;
use crate::carry::{IN_FRONT, TURN_SECONDS};

#[test]
fn a_carried_panel_stays_in_front_of_a_curved_panel() {
    let (mut space, now) = space();
    space.panels[0].bend = 1.5;
    let window = carry_popped_out(&mut space, now);
    let now = later(now, TURN_SECONDS);
    for x in [120.0, 200.0, 400.0] {
        let at = pos2(x, centre().y);
        send(&mut space, now, [Input::PointerMoved(at)]);
        let gaps = gaps_to(&space, window, WindowId::MAIN);
        assert!(gaps.len() > 3, "it is over the curved panel at {x}");
        assert!(
            gaps.iter().all(|gap| *gap >= IN_FRONT - 2e-3),
            "in front of the curved surface where it comes nearer, at {x}: {gaps:?}"
        );
        let ray = space.head.ray(PLATFORM, at.to_vec2());
        assert_eq!(space.pick(ray).map(|hit| hit.window), Some(window));
    }
}
