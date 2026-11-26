use super::*;

/// The holder's layer is committing, and the order passes over the entry it goes out as (it
/// was based below a barrier): the peer stops showing it then, as it never will land.
#[test]
fn a_peer_layer_whose_commit_the_order_passes_over_leaves() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    let moved = Checklist::set_text(a, "moved");
    let through = super::tentative::journaled_through(&sim, 0) + 1;
    super::tentative::relay_held(&mut sim, 1, 0, 1, &[moved.clone()], Some(through));
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "b"]);

    let base = super::tentative::barrier(&mut sim, 0);
    let entry = super::tentative::inline(&moved);
    super::tentative::number_as(&mut sim, 0, through, entry, base, false);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);
}
