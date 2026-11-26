use super::*;

/// The entry that brings the holder's origin to where its layer commits is a stash it keeps:
/// the layer leaves with it.
#[test]
fn a_peer_layer_committing_with_a_kept_stash_leaves_with_it() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    let through = super::tentative::journaled_through(&sim, 0) + 1;
    super::tentative::relay_held(&mut sim, 1, 0, 1, &[Checklist::set_text(a, "moved")], Some(through));
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "b"]);

    let base = sim.order.sequence;
    let entry = Entry::Keep {
        commit: CommitId::from_hash(Hash::of(b"a stash")),
    };
    super::tentative::number_as(&mut sim, 0, through, entry, base, false);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
}
