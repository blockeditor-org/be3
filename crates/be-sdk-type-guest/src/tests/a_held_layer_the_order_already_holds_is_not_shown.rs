use super::*;

/// A `Held` saying its layer commits through edits the order already holds (its entry came
/// first) shows nothing.
#[test]
fn a_held_layer_the_order_already_holds_is_not_shown() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    let through = super::tentative::journaled_through(&sim, 0);
    super::tentative::relay_held(
        &mut sim,
        1,
        0,
        1,
        &[Checklist::set_text(a, "late")],
        Some(through),
    );
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
}
