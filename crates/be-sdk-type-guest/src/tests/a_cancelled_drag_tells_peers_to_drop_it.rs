use super::*;

/// Cancelling a held drag tells the peers to drop the layer, and a drag that follows is told
/// as a layer of its own, from its first edit.
#[test]
fn a_cancelled_drag_tells_peers_to_drop_it() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    super::tentative::told_held(&mut sim, 0);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "1"));
    sim.operate(0, 1, Checklist::set_text(a, "2"));
    sim.settle();
    super::tentative::told_held(&mut sim, 0);
    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    assert_eq!(
        super::tentative::told_held(&mut sim, 0),
        [(0, 0, Vec::new(), None)]
    );

    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "3"));
    sim.settle();
    assert_eq!(
        super::tentative::told_held(&mut sim, 0),
        [(0, 1, vec![Checklist::set_text(a, "3")], None)]
    );
}
