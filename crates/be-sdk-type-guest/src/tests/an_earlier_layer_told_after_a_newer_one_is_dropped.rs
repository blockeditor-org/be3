use super::*;

/// A `Held` about an earlier layer of a peer than the one shown is dropped.
#[test]
fn an_earlier_layer_told_after_a_newer_one_is_dropped() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    super::tentative::relay_held(&mut sim, 1, 0, 2, &[Checklist::set_text(a, "two")], None);
    super::tentative::relay_held(&mut sim, 1, 0, 1, &[Checklist::set_text(a, "one")], None);
    assert_eq!(texts(sim.mirror(1, 1)), ["two", "b"]);
}
