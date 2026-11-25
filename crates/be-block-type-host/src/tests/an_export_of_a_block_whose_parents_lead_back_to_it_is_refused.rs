use be_type_example_counter::CounterContent;

use super::*;

/// Offline, a block moved into the block inside it is kept so on this device until the server
/// refuses the move as the outbox drains: its parents lead back to it. An export of the inner
/// block meets the outer one again above it, and is refused rather than walk up for ever.
#[test]
fn an_export_of_a_block_whose_parents_lead_back_to_it_is_refused() {
    with_test_types(|peer, _shared, mut replicas| async move {
        let outer = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the outer block is made");
        let inner = peer
            .create::<CounterContent>(BlockParent::Block(outer))
            .await
            .expect("the inner block is made");
        peer.detach();
        peer.set_parent(outer, BlockParent::Block(inner))
            .await
            .expect("offline, the move is kept on this device");

        let exported =
            crate::transfer::collect(&peer, inner, |_| false, &mut replicas.modules).await;
        let Err(be_block_client::ClientError::Refused(_, why)) = exported else {
            panic!("an export through a cycle is refused: {exported:?}");
        };
        assert!(
            why.contains("listed inside itself"),
            "it says why: {why}"
        );
    });
}
