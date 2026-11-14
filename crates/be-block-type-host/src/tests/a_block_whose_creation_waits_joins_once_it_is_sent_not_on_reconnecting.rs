use super::*;

/// A block opened while its creation waits in the outbox joins its session once the drain sends
/// that, not when the device reconnects before it has: the server does not know the block yet.
#[test]
fn a_block_whose_creation_waits_joins_once_it_is_sent_not_on_reconnecting() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = Uuid::new_v4();
        peer.keep_creation(
            block,
            Sections::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .expect("the block is kept here");
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        assert!(
            !replicas.joining(),
            "a block the server does not know opens as offline does"
        );

        replicas.reconnect((&peer, &shared));
        assert!(
            !replicas.joining(),
            "reconnecting left alone a block whose creation was not sent"
        );

        peer.drain(|_| false).await.expect("the creation is sent");
        assert!(!peer.creating(block));
        replicas.created((&peer, &shared));
        assert!(replicas.joining(), "once sent, the block joins its session");
    });
}
