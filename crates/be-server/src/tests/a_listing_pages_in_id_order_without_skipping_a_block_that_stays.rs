use super::*;

async fn page(
    client: &mut TestClient,
    parent: Uuid,
    after: Option<Uuid>,
) -> be_block_protocol::Listed {
    client
        .call(|peer| {
            client::blocks::list_blocks(peer, Listing::Children(BlockParent::Block(parent)), after)
        })
        .await
        .expect("listing failed")
}

/// A listing comes in pages in the order of block ids, each going on past the last: a block
/// that stays is on exactly one page, one removed before its page is on none, and the pages end.
#[tokio::test]
async fn a_listing_pages_in_id_order_without_skipping_a_block_that_stays() {
    let harness = Harness::configured(ServerConfig {
        listing_page_bytes: 512,
        ..ServerConfig::OPEN
    })
    .await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.organization("pages").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let mut children = Vec::new();
    for _ in 0..40 {
        children.push(owner.create_block(BlockParent::Block(folder)).await);
    }
    children.sort();

    let first = page(&mut owner, folder, None).await;
    assert!(first.next.is_some(), "forty blocks fit one page");
    let mut seen: Vec<Uuid> = first.blocks.iter().map(|summary| summary.id).collect();
    seen.extend(&first.rest);
    assert!(
        seen.windows(2).all(|pair| pair[0] < pair[1]),
        "a page is in id order"
    );
    // Between pages, the last child, not yet listed, moves out, and a listed one moves out too.
    let gone = *children.last().unwrap();
    assert!(!seen.contains(&gone));
    let moved = owner.move_to(gone, BlockParent::Root).await;
    assert!(moved.is_ok(), "{moved:?}");
    let left_listed = seen[0];
    let moved = owner.move_to(left_listed, BlockParent::Root).await;
    assert!(moved.is_ok(), "{moved:?}");

    let mut after = first.next;
    let mut pages = 1;
    while let Some(cursor) = after {
        let next = page(&mut owner, folder, Some(cursor)).await;
        pages += 1;
        assert!(pages < 100, "the pages never ended");
        let ids: Vec<Uuid> = next
            .blocks
            .iter()
            .map(|summary| summary.id)
            .chain(next.rest.iter().copied())
            .collect();
        assert!(ids.iter().all(|id| *id > cursor), "a page went back");
        seen.extend(ids);
        after = next.next;
    }
    assert!(pages > 2, "only {pages} pages");
    let expected: Vec<Uuid> = children
        .iter()
        .copied()
        .filter(|child| *child != gone)
        .collect();
    assert_eq!(seen, expected, "each block that stayed, once, in order");

    harness.stop().await;
}
