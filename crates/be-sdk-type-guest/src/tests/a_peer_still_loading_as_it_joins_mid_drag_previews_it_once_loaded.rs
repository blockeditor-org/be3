use super::*;

/// A device that joins mid-drag while it is still loading previews the drag once it loads,
/// even one held still: what it is told as it joins waits for the load, as all it hears does.
#[test]
fn a_peer_still_loading_as_it_joins_mid_drag_previews_it_once_loaded() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let (a, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.settle();
    // Sealed, so a replica opening loads the head.
    sim.advance(crate::QUIET_INTERVAL + 1);
    sim.settle();
    assert!(sim.head.is_some());
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "held"));
    sim.settle();

    sim.load_stalled = true;
    sim.open(1);
    sim.attach(1, 1, true);
    sim.settle();
    assert!(!sim.held.is_empty(), "its load waits");
    assert!(
        sim.editors[&(1, 1)].content().is_none(),
        "it shows nothing yet"
    );
    sim.unstall();
    assert_eq!(texts(sim.mirror(1, 1)), ["held"]);
}
