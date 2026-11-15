use super::*;

/// A listing page names its last blocks by id alone (`listed.rest`), without their summaries,
/// so the connection was never told their references: a change to one is announced with every
/// reference, not as what it did to them.
#[tokio::test]
async fn a_change_to_a_block_listed_only_by_id_comes_whole() {
    let harness = Harness::configured(ServerConfig {
        listing_page_bytes: 512,
        ..ServerConfig::OPEN
    })
    .await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("pages").await;
    let folder = owner.create_block(BlockParent::Root).await;
    for _ in 0..40 {
        owner.create_block(BlockParent::Block(folder)).await;
    }

    let mut watcher = harness
        .second_connection(&owner.token.clone(), organization)
        .await;
    let page = watcher
        .call(|peer| {
            client::blocks::list_blocks(peer, Listing::Children(BlockParent::Block(folder)), None)
        })
        .await
        .expect("listing failed");
    let summarized = page.blocks[0].id;
    let named = *page
        .rest
        .first()
        .expect("forty blocks fit one page's summaries");

    for (block, whole) in [(summarized, false), (named, true)] {
        let renamed = owner
            .call(|peer| client::blocks::set_metadata(peer, block, b"renamed".to_vec(), Vec::new()))
            .await;
        assert!(renamed.is_ok(), "{renamed:?}");
        let Notice::BlockChanged {
            block: told, moved, ..
        } = watcher.graph_change().await
        else {
            panic!("the watcher heard something other than the rename");
        };
        assert_eq!(told.id, block);
        assert_eq!(
            moved.is_none(),
            whole,
            "a block summarized is told what moved, one named by id alone is told whole"
        );
    }

    harness.stop().await;
}
