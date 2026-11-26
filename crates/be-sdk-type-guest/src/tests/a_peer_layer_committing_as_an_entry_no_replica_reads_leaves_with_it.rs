use super::*;

/// The holder's layer commits as an entry every replica passes over unread (its part is too
/// long to read), though the order holds it: the layer leaves with it all the same.
#[test]
fn a_peer_layer_committing_as_an_entry_no_replica_reads_leaves_with_it() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    let through = super::tentative::journaled_through(&sim, 0) + 1;
    super::tentative::relay_held(
        &mut sim,
        1,
        0,
        1,
        &[Checklist::set_text(a, "moved")],
        Some(through),
    );
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "b"]);

    let part = be_block_store::PartRef::Chunks(be_block_store::NodeRef {
        hash: Hash::of(b"never stored"),
        length: 1 << 40,
    });
    let base = sim.order.sequence;
    let entry = Entry::Operation(be_block_session::Payload::Part(part));
    super::tentative::number_as(&mut sim, 0, through, entry, base, false);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
}
