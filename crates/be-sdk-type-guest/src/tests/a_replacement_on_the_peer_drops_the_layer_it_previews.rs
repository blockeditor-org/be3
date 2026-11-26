use super::*;

/// The peer replaces the content mid-drag: what it previewed of the drag was made on what the
/// replacement took away, and leaves with it, before the holder hears of it.
#[test]
fn a_replacement_on_the_peer_drops_the_layer_it_previews() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "dragged"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["dragged", "b"]);

    sim.deaf.insert(0);
    let mut replacement = sim.content(1);
    replacement.apply(&Checklist::set_text(a, "replaced"));
    sim.send_from_editor(1, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["replaced", "b"]);
}
