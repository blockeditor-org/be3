use super::*;

/// A replica whose load meets a head's objects gone opens again, in case the head moved on (a
/// head another device replaced, which the server dropped); if what it loads then, once the join
/// answered, is that same commit missing again, it is gone for good and the replica is
/// unreadable, rather than opening again and again or waiting for ever.
#[test]
fn a_head_gone_everywhere_opens_once_more_then_is_unreadable() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = block(&peer).await;
        let saved = peer
            .save(block, &base(), None)
            .await
            .expect("the block is written");
        let be_block_client::Saved::Published(head) = saved else {
            panic!("the head reached the server: {saved:?}");
        };
        lose_everywhere(head.hash());

        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        let openings = |replicas: &Replicas<Quiet>| {
            replicas
                .openings
                .iter()
                .filter(|(opened, ..)| *opened == block)
                .count()
        };
        let unreadable = loop {
            if let Some(reason) = replicas.status(block).and_then(|status| status.unreadable) {
                break reason;
            }
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        };
        assert!(
            unreadable.contains("is nowhere to be found"),
            "{unreadable}"
        );
        assert_eq!(
            openings(&replicas),
            2,
            "it opened once more for the missing head, and no more"
        );
    });
}
