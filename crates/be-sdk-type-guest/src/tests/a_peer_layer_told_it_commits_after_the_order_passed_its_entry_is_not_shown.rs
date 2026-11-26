use super::*;

/// The order passes over the entry a holder's layer goes out as before the peer is told the
/// layer commits as it: the peer never shows it, as it never will land.
#[test]
fn a_peer_layer_told_it_commits_after_the_order_passed_its_entry_is_not_shown() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    let moved = Checklist::set_text(a, "moved");
    let through = super::tentative::journaled_through(&sim, 0) + 1;
    let base = super::tentative::barrier(&mut sim, 0);
    let entry = super::tentative::inline(&moved);
    super::tentative::number_as(&mut sim, 0, through, entry, base, false);

    super::tentative::relay_held(&mut sim, 1, 0, 1, &[moved], Some(through));
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
}
