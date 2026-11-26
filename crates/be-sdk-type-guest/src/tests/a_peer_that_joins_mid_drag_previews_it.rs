use super::*;

/// A device that joins while another holds a drag previews it: told the layer whole as it
/// joins, or, when that came before it could show it, from the drag's next frame, which it
/// asks the whole for.
#[test]
fn a_peer_that_joins_mid_drag_previews_it() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let (a, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "1"));
    sim.operate(0, 1, Checklist::set_text(a, "2"));
    sim.settle();

    sim.open(1);
    sim.attach(1, 1, true);
    sim.settle();
    sim.operate(0, 1, Checklist::set_text(a, "3"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["3"]);
    assert_eq!(texts(&sim.content(1)), ["a"]);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["3"]);
    assert_eq!(texts(&sim.content(1)), ["3"]);
}
