use super::*;

/// The device holding a drag tells one that joins the layer as it sees it join, so the
/// joiner previews the drag at once, even when it could not ask for it.
#[test]
fn a_peer_joining_mid_drag_is_told_the_layer_as_it_joins() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let (a, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "held"));
    sim.settle();

    // Its asks go unheard.
    sim.deaf.insert(0);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["held"]);
    sim.deaf.clear();
}
