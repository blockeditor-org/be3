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

    let response = client
        .move_keyed(deleted, BlockParent::Detached, KeySet::default())
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
        .move_keyed(deleted, BlockParent::Detached, KeySet::default())
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(!client.read_block(deleted).await.held);
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
    let root = client.root(block);
    let placement = client.placement(block, parent).await;
    client
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::CreateBlock { block: block, content_type: content_type, parent: parent, metadata: Vec::new(), references: Vec::new(), key: key, keys: keys, owner_only: false, root: root, placement: placement })
        })
        .await
}
