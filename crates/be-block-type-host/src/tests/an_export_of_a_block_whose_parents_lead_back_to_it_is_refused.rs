use be_type_example_counter::CounterContent;

use super::*;

/// Offline, this device moves the outer block into the inner one, which it last heard was at the
/// root, while another device moved the inner block into the outer one. Online again before its
/// move is sent, it hears the inner block's new parent, and keeps its own move laid over what
/// the server says: its parents lead back to it. An export of the inner block offline meets the
/// outer one again above it, and is refused rather than walk up for ever.
#[test]
fn an_export_of_a_block_whose_parents_lead_back_to_it_is_refused() {
    with_another_device(|peer, another, _shared, mut replicas| async move {
        let outer = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the outer block is made");
        let inner = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the inner block is made");
        let (connection, token) = (peer.connection(), peer.token());
        peer.detach();
        another
            .set_parent(inner, BlockParent::Block(outer))
            .await
            .expect("the other device moves the inner block into the outer one");
        peer.set_parent(outer, BlockParent::Block(inner))
            .await
            .expect("offline, the move is kept on this device");
        peer.attach(connection, token)
            .await
            .expect("the device is connected again");
        peer.summary(inner)
            .await
            .expect("it hears where the inner block is now");
        peer.detach();

        let exported =
            crate::transfer::collect(&peer, inner, |_| false, &mut replicas.modules).await;
        let Err(be_block_client::ClientError::Refused(_, why)) = exported else {
            panic!("an export through a cycle is refused: {exported:?}");
        };
        assert!(why.contains("listed inside itself"), "it says why: {why}");
    });
}
