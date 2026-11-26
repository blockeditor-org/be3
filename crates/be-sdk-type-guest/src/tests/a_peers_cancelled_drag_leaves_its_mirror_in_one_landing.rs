use super::*;

/// A peer's drag cancelled leaves the mirror showing what it showed before, in one landing that
/// lays what is unsettled again, with no snapshot.
#[test]
fn a_peers_cancelled_drag_leaves_its_mirror_in_one_landing() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "1"));
    sim.operate(0, 1, Checklist::set_text(a, "2"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["2", "b"]);
    super::tentative::landings(&mut sim, 1, 1);

    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
    let (snapshots, landed) = super::tentative::landings(&mut sim, 1, 1);
    assert_eq!(snapshots, 0);
    assert_eq!(landed.len(), 1);
    assert!(matches!(&landed[0].landed, Landed::Relaid { local, .. } if local.is_empty()));
}
