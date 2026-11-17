use super::*;
use be_block_protocol::Listing;

#[tokio::test]
async fn a_blocks_references_are_listed_and_announced_together() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("tracker@example.com").await;
    let organization = client.organization("issues").await;
    let author = Author::new(account);

    let view = client.create_block(BlockParent::Root).await;
    let mut issues = Vec::new();
    for _ in 0..3 {
        issues.push(client.create_block(BlockParent::Root).await);
    }
    let unlinked = client.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"view", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(commit, &chunks))
        .await;
    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                view,
                commit,
                None,
                Vec::new(),
                objects_change(&[], chunks),
                1_000,
                false,
                listing(&[], issues.clone()),
                Vec::new(),
                Vec::new(),
                None,
            )
        })
        .await;
    assert!(
        matches!(
            response,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{response:?}"
    );

    let mut other = harness
        .second_connection(&client.token.clone(), organization)
        .await;
    other.read_block(view).await;
    let response = other
        .call(|peer| client::blocks::list_blocks(peer, Listing::References(view), None))
        .await;
    let Ok(be_block_protocol::Listed { blocks, .. }) = response else {
        panic!("{response:?}");
    };
    let mut listed: Vec<Uuid> = blocks.iter().map(|block| block.id).collect();
    listed.sort_unstable();
    let mut expected = issues.clone();
    expected.sort_unstable();
    assert_eq!(listed, expected, "one request lists every linked block");

    // A block a publish stops referencing is told to the readers of its tree, whose summary of
    // it says who may know of it through the publish's block.
    other.follow(issues[1]).await;
    other.follow(unlinked).await;
    let kept: Vec<Uuid> = [issues[0], issues[2]].into();
    let (next, chunks) = author.compose(b"view again", 2_000, Some(commit));
    client
        .upload(&author.store, &author.objects_of(next, &chunks))
        .await;
    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                view,
                next,
                Some(commit),
                vec![commit],
                objects_change(&author.held_by(Some(commit)), chunks),
                2_000,
                false,
                listing(&issues, kept),
                Vec::new(),
                Vec::new(),
                None,
            )
        })
        .await;
    assert!(response.is_ok(), "{response:?}");
    let entry = loop {
        if let Notice::TreeAdvanced { entry, .. } = other.graph_change().await {
            break entry;
        }
    };
    assert_eq!(
        entry, issues[1],
        "an unlinked block's tree advances, and a block the publish left alone does not"
    );

    harness.stop().await;
}
