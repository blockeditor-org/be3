use super::*;

/// A block read before it exists is announced once it is made, so a refused listing is kept; but
/// only `REFUSED_LISTINGS` of them, so reading made-up ids holds no more of the server's memory.
#[tokio::test]
async fn a_connection_keeps_only_so_many_refused_listings() {
    let harness = Harness::start().await;
    let mut maker = harness.client().await;
    maker.register("maker@example.com").await;
    let organization = maker.organization("notes").await;
    let token = maker.token.clone();
    let mut reader = harness.second_connection(&token, organization).await;

    let early: Vec<Uuid> = (0..crate::watch::REFUSED_LISTINGS)
        .map(|_| Uuid::new_v4())
        .collect();
    for block in &early {
        let read = reader
            .call(|peer| client::blocks::read_block(peer, *block))
            .await;
        assert!(refused_as(&read, ErrorCode::BlockNotFound), "{read:?}");
    }
    let late = Uuid::new_v4();
    let read = reader
        .call(|peer| client::blocks::read_block(peer, late))
        .await;
    assert!(refused_as(&read, ErrorCode::BlockNotFound), "{read:?}");

    for block in [late, early[0]] {
        let key = Uuid::new_v4();
        maker
            .create_keyed(
                block,
                BlockParent::Detached,
                key,
                KeySet {
                    keys: vec![row(key, block, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
            )
            .await;
    }
    let Notice::BlockChanged { block: heard } = reader.graph_change().await else {
        panic!("the block read before it was made was not announced");
    };
    assert_eq!(
        heard.id, early[0],
        "a listing past the bound was kept and announced"
    );

    harness.stop().await;
}
