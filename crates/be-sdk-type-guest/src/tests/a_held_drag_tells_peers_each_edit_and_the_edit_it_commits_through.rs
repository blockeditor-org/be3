use super::*;

/// Each frame of a held drag goes to the peers as one `Held`, appended to what they hold, and
/// its commit as one naming the last local edit the layer goes out as, which the order holds
/// once it lands.
#[test]
fn a_held_drag_tells_peers_each_edit_and_the_edit_it_commits_through() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    super::tentative::told_held(&mut sim, 0);
    sim.gesture(0, 1, Gesture::BeginTentative);
    for frame in ["1", "2", "3"] {
        sim.operate(0, 1, Checklist::set_text(a, frame));
        sim.settle();
    }
    let told = super::tentative::told_held(&mut sim, 0);
    assert_eq!(
        told,
        [
            (0, 1, vec![Checklist::set_text(a, "1")], None),
            (1, 2, vec![Checklist::set_text(a, "2")], None),
            (2, 3, vec![Checklist::set_text(a, "3")], None),
        ]
    );

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    let told = super::tentative::told_held(&mut sim, 0);
    let through = super::tentative::journaled_through(&sim, 0);
    assert_eq!(told, [(3, 3, Vec::new(), Some(through))]);
    assert_eq!(texts(&sim.content(1)), ["3", "b"]);
}
