use super::*;
use crate::carry::{IN_FRONT, TURN_SECONDS};

fn let_go(after: f32) -> (Space, WindowId) {
    let (mut space, now) = space();
    let popped = carry_popped_out(&mut space, now);
    let to = centre() + vec2(250.0, 150.0);
    let moved = later(now, 0.1);
    send(&mut space, moved, [Input::PointerMoved(to)]);
    let turning = facing_eye(&space, space.panel(popped).unwrap());
    assert!(
        turning > 0.05,
        "a tenth of a second in, it is still turning: {turning}"
    );
    let released = send(&mut space, later(now, after), [button(to, false)]);
    assert_eq!(space.mouse.carry, None);
    assert_eq!(placed(&released).len(), 1);
    (space, popped)
}

#[test]
fn a_carried_panel_let_go_mid_turn_lands_where_it_would_after_the_turn() {
    let (early, popped) = let_go(0.1);
    let (late, late_popped) = let_go(TURN_SECONDS + 0.1);
    let (early_panel, late_panel) = (
        early.panel(popped).unwrap(),
        late.panel(late_popped).unwrap(),
    );
    assert!(
        close(early_panel.pose.position, late_panel.pose.position)
            && early_panel
                .pose
                .rotation
                .angle_between(late_panel.pose.rotation)
                < 1e-3,
        "let go mid-turn it finishes the turn at once: {:?} and {:?}",
        early_panel.pose,
        late_panel.pose
    );
    assert!(facing_eye(&early, early_panel) < 1e-3);
    let gaps = gaps_to(&early, popped, WindowId::MAIN);
    assert!(
        !gaps.is_empty() && gaps.iter().all(|gap| *gap >= IN_FRONT - 1e-3),
        "its distance is solved again for the turn it finished, so it stays in front: {gaps:?}"
    );
}
