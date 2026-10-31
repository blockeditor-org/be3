use super::*;

async fn rename(client: &mut TestClient, block: Uuid, name: &[u8]) {
    let response = client
        .call(|peer| client::blocks::set_metadata(peer, block, name.to_vec(), Vec::new()))
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
}

async fn heard(client: &mut TestClient) -> Uuid {
    match client.graph_change().await {
        Notice::BlockChanged { block } => block.id,
        Notice::BlockRemoved { block, .. } => block,
        other => panic!("{other:?}"),
    }
}

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
                chunks,
                1_000,
                false,
                issues.clone(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
        "{response:?}"
    );

    let mut other = harness
        .second_connection(&client.token.clone(), organization)
        .await;
    other.read_block(view).await;
    let response = other
        .call(|peer| client::blocks::list_blocks(peer, Listing::References(view)))
        .await;
    let Ok(blocks) = response else {
        panic!("{response:?}");
    };
    let mut listed: Vec<Uuid> = blocks.iter().map(|block| block.id).collect();
    listed.sort_unstable();
    let mut expected = issues.clone();
    expected.sort_unstable();
    assert_eq!(listed, expected, "one request lists every linked block");

    rename(&mut client, unlinked, b"unheard").await;
    rename(&mut client, issues[1], b"renamed").await;
    assert_eq!(
        heard(&mut other).await,
        issues[1],
        "a linked block's change is announced and an unlinked one's is not"
    );

    harness.stop().await;
}
