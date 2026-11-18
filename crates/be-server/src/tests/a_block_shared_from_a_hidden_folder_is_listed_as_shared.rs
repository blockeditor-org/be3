use super::*;
use be_block_graph::Access;

async fn listed(client: &mut TestClient, shared: bool) -> Vec<Uuid> {
    let response = client
        .call(|peer| {
            peer.read::<be_block_protocol::Listed>(be_block_protocol::Query::List {
                listing: match shared {
                    true => Listing::Shared,
                    false => Listing::Children(BlockParent::Root),
                },
                after: None,
            })
        })
        .await;
    let Ok(be_block_protocol::Listed { blocks, .. }) = response else {
        panic!("listing failed: {response:?}");
    };
    let mut ids: Vec<Uuid> = blocks.into_iter().map(|block| block.id).collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn a_block_shared_from_a_hidden_folder_is_listed_as_shared() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let inside = owner.create_block(BlockParent::Block(folder)).await;
    owner.create_block(BlockParent::Block(inside)).await;

    let mut guest = harness.member("guest@example.com").await;
    let response = owner
        .invite_with(Uuid::from_u128(1), organization, Vec::new(), Vec::new())
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = guest.accept(Uuid::from_u128(1), &Vec::new()).await;
    guest.open(organization).await;
    let account = guest.id();
    let own = guest.create_block(BlockParent::Root).await;
    assert_eq!(listed(&mut guest, false).await, [own]);
    assert!(listed(&mut guest, true).await.is_empty());

    let response = owner.grant(inside, account, Access::View).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(
        listed(&mut guest, true).await,
        [inside],
        "a block shared from a folder the guest cannot see is listed as shared, without what is inside it"
    );
    assert_eq!(listed(&mut guest, false).await, [own]);

    let response = owner.grant(folder, account, Access::View).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(
        listed(&mut guest, true).await.is_empty(),
        "once its folder is visible, the block is reached through the folder"
    );
    let mut roots = vec![own, folder];
    roots.sort();
    assert_eq!(listed(&mut guest, false).await, roots);
    assert!(
        listed(&mut owner, true).await.is_empty(),
        "an administrator sees every folder, so nothing is shared into view"
    );

    harness.stop().await;
}
