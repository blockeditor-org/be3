use super::*;

/// A long drag is told as appends for a few frames, then whole, absorbed, so what a peer holds
/// of it stays a few edits however long it goes on, and the peer follows every frame.
#[test]
fn a_long_drag_is_told_whole_every_few_frames() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    super::tentative::told_held(&mut sim, 0);
    sim.gesture(0, 1, Gesture::BeginTentative);
    for frame in 0..60 {
        let text = format!("{frame}");
        sim.operate(0, 1, Checklist::set_text(a, &text));
        sim.settle();
        assert_eq!(texts(sim.mirror(1, 1)), [text.as_str(), "b"]);
    }
    let told = super::tentative::told_held(&mut sim, 0);
    assert_eq!(told.len(), 60, "one Held a frame");
    let whole: Vec<_> = told.iter().filter(|(from, ..)| *from == 0).collect();
    assert!(
        whole.len() >= 3,
        "told whole every few frames: {}",
        whole.len()
    );
    for (_, to, operations, _) in &whole {
        assert_eq!(operations.len(), 1, "the {to} edits absorbed into one");
    }
    let (snapshots, _) = super::tentative::landings(&mut sim, 1, 1);
    assert_eq!(snapshots, 0);
}
