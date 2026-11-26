use super::*;
use be_block_session::SessionMessage;

/// A peer asking for the layer whole again and again is told it once for each state of the
/// layer, so asking costs the holder nothing more.
#[test]
fn asking_again_and_again_is_answered_once_per_frame() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "1"));
    sim.settle();
    let asker = sim.devices[1].client;
    let ask = |sim: &mut Sim<ChecklistContent>| {
        for _ in 0..3 {
            sim.relay_as(0, asker, crate::wire::encode(&SessionMessage::HeldAgain));
            sim.settle();
        }
        super::tentative::told_held(sim, 0).len()
    };
    super::tentative::told_held(&mut sim, 0);
    assert_eq!(ask(&mut sim), 1);
    sim.operate(0, 1, Checklist::set_text(a, "2"));
    sim.settle();
    super::tentative::told_held(&mut sim, 0);
    assert_eq!(ask(&mut sim), 1, "told again once the layer changed");
}
