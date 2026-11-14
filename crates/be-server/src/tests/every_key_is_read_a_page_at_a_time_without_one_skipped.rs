use super::*;

#[tokio::test]
async fn every_key_is_read_a_page_at_a_time_without_one_skipped() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.organization("keys").await;
    // Each block's key row comes to tens of bytes, so these take several of the test's pages.
    let mut made = Vec::new();
    for _ in 0..200 {
        made.push(owner.create_block(BlockParent::Root).await);
    }

    let first = owner
        .call(|peer| client::keys::get_keys(peer, None, None))
        .await
        .unwrap();
    assert!(first.next.is_some(), "the keys fit one page");
    assert!(be_block_protocol::encoded_len(&first.keys) <= 4 * 1024);

    let keys = owner.keys(None).await;
    let mut ids: Vec<Uuid> = keys.keys.iter().map(|row| row.id).collect();
    let pages = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), pages, "a key was read twice");
    let blocks: std::collections::BTreeSet<Uuid> = keys.keys.iter().map(|row| row.block).collect();
    for block in made {
        assert!(blocks.contains(&block), "a block's key was skipped");
    }

    harness.stop().await;
}
