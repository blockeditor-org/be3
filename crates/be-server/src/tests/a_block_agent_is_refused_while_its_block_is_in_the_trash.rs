use super::*;

#[tokio::test]
async fn a_block_agent_is_refused_while_its_block_is_in_the_trash() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let me = owner.register("owner@example.com").await;
    let organization = owner.organization("team").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let terminal = owner.create_block(BlockParent::Block(folder)).await;
    // A child is in the trash unless its parent holds it.
    let author = Author::new(me);
    let (head, chunks) = author.compose(b"the folder", 1_000, None);
    owner
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let published = owner
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block: folder,
                commit: head,
                expected: None,
                parents: Vec::new(),
                held: objects_change(&[], chunks.clone()),
                time: 1_000,
                pinned: false,
                references: listing(&[], vec![terminal]),
                metadata: Vec::new(),
                objects: Vec::new(),
                journal: None,
            })
        })
        .await;
    assert!(published.is_ok(), "{published:?}");
    let (_, token) = owner.block_agent(organization, terminal).await.unwrap();

    for (trashed, parent) in [
        (terminal, BlockParent::Block(folder)),
        (folder, BlockParent::Root),
    ] {
        let mut agent = harness.second_connection(&token, organization).await;
        let response = owner
            .move_keyed(trashed, BlockParent::Detached, KeySet::default())
            .await;
        assert!(response.is_ok(), "{response:?}");
        agent.ended().await;

        let mut again = harness.client().await;
        let signing = token.clone();
        // Signed in without listening: following its contacts would be refused too.
        let response = client::server::authenticate(&again.peer, Some(signing))
            .await
            .expect("the connection stayed open");
        assert!(
            response.is_ok(),
            "the trash deleted the block's agent: {response:?}"
        );
        let response = again
            .call(|peer| {
                peer.read_in::<be_block_protocol::Listed>(
                    Some(organization),
                    Query::List {
                        listing: Listing::Children(BlockParent::Root),
                        after: None,
                    },
                )
            })
            .await;
        assert!(
            refused_as(&response, ErrorCode::PermissionDenied),
            "an agent read its organization while its block is in the trash: {response:?}"
        );

        owner
            .move_to(trashed, parent)
            .await
            .expect("restoring the block failed");
    }
    let mut agent = harness.second_connection(&token, organization).await;
    let response = agent.try_read_block(terminal).await;
    assert!(
        response.is_ok(),
        "a restored block's agent is still refused: {response:?}"
    );

    harness.stop().await;
}
