use super::*;

/// An account's Unfiled has no content listing its blocks, yet holds each block whose parent
/// names it: making one there edits nothing, a delete is a move out of it, and a restore moves it
/// back.
#[tokio::test]
async fn a_block_in_unfiled_stays_live_without_a_listing_until_it_is_deleted() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("filer@example.com").await;
    let organization = client.organization("notes").await;
    let unfiled = be_block::unfiled(organization, account);
    let made = make(
        &mut client,
        unfiled,
        be_block::UNFILED_TYPE,
        BlockParent::Root,
    )
    .await;
    assert!(made.is_ok(), "making Unfiled failed: {made:?}");

    let kept = client.create_block(BlockParent::Block(unfiled)).await;
    let deleted = client.create_block(BlockParent::Block(unfiled)).await;
    assert!(client.read_block(kept).await.held);
    assert!(client.read_block(deleted).await.held);
    assert!(client.read_block(unfiled).await.references.is_empty());

    let collected = collect(&mut client).await;
    assert!(collected.is_empty(), "nothing was deleted: {collected:?}");

    let response = client
        .call(|peer| client::blocks::delete_block(peer, deleted))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(!client.read_block(deleted).await.held);
    let response = client.move_to(deleted, BlockParent::Block(unfiled)).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(
        client.read_block(deleted).await.held,
        "restored to Unfiled, it is live again"
    );

    let response = client
        .call(|peer| client::blocks::delete_block(peer, deleted))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(collect(&mut client).await, vec![deleted]);
    assert!(client.read_block(kept).await.held);

    let response = client.move_to(unfiled, BlockParent::Detached).await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "Unfiled stays at the top: {response:?}"
    );

    let harness = harness.restart().await;
    let mut client = harness.second_connection(&client.token, organization).await;
    assert!(
        client.read_block(kept).await.held,
        "the server keeps which block is an Unfiled across a restart"
    );

    harness.stop().await;
}

pub(super) async fn make(
    client: &mut TestClient,
    block: Uuid,
    content_type: Uuid,
    parent: BlockParent,
) -> Result<BlockSummary, Error> {
    let key = Uuid::new_v4();
    let keys = KeySet {
        keys: vec![row(key, block, KeyKind::Data)],
        wrapped: Vec::new(),
    };
    client
        .call(|peer| {
            client::blocks::create_block(
                peer,
                block,
                content_type,
                parent,
                Vec::new(),
                Vec::new(),
                key,
                keys,
                false,
            )
        })
        .await
}

async fn collect(client: &mut TestClient) -> Vec<Uuid> {
    let response = client
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    let Ok(be_block_protocol::Collected { blocks, .. }) = response else {
        panic!("collection failed: {response:?}");
    };
    blocks
}
