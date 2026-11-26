use super::*;
use be_block_session::SessionMessage;

/// A peer that missed a frame of a drag cannot append the next to what it holds: it asks for
/// the layer whole, once, and shows it from that frame on.
#[test]
fn a_peer_that_missed_a_frame_asks_for_the_layer_whole() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "1"));
    sim.settle();
    sim.deaf.insert(1);
    sim.operate(0, 1, Checklist::set_text(a, "2"));
    sim.settle();
    sim.deaf.clear();
    assert_eq!(texts(sim.mirror(1, 1)), ["1", "b"]);

    sim.sent.clear();
    sim.operate(0, 1, Checklist::set_text(a, "3"));
    sim.settle();
    let asked = sim
        .sent
        .iter()
        .filter(|(from, bytes)| {
            *from == 1 && crate::wire::decode(bytes) == Some(SessionMessage::HeldAgain)
        })
        .count();
    assert_eq!(asked, 1);
    assert_eq!(texts(sim.mirror(1, 1)), ["3", "b"]);
    sim.operate(0, 1, Checklist::set_text(a, "4"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["4", "b"]);
}
