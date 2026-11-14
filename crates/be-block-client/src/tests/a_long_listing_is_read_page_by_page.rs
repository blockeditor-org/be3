use super::*;
use be_type_text::TextContent;

/// The server pages a long listing in the order of block ids; the peer reads every page, and a
/// block's references come back in the block's own order.
#[tokio::test]
async fn a_long_listing_is_read_page_by_page() {
    let harness = Harness::configured(be_server::ServerConfig {
        listing_page_bytes: 512,
        ..be_server::ServerConfig::OPEN
    })
    .await;
    let author = harness.owner("pager@example.com").await;
    let workspace = workspace(&author, "Pages").await;
    let mut children = Vec::new();
    for _ in 0..40 {
        children.push(
            author
                .create::<TextContent>(BlockParent::Block(workspace))
                .await
                .unwrap(),
        );
    }
    let first = crate::ask(be_block_protocol::client::blocks::list_blocks(
        &author.connection(),
        Listing::Children(BlockParent::Block(workspace)),
        None,
    ))
    .await
    .unwrap();
    assert!(first.next.is_some(), "the listing fit one page");

    let reader = harness.second(&author).await;
    let mut listed: Vec<Uuid> = reader
        .list(Listing::Children(BlockParent::Block(workspace)))
        .await
        .unwrap()
        .iter()
        .map(|summary| summary.id)
        .collect();
    listed.sort();
    let mut made = children.clone();
    made.sort();
    assert_eq!(listed, made);

    // References in an order of their own, not of their ids.
    let referrer = author
        .create::<TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let mut order = children.clone();
    order.sort();
    order.reverse();
    order.swap(3, 30);
    let referred = crate::ask(be_block_protocol::client::blocks::set_metadata(
        &author.connection(),
        referrer,
        Vec::new(),
        order.clone(),
    ))
    .await;
    assert!(referred.is_ok(), "{referred:?}");
    let references: Vec<Uuid> = reader
        .list(Listing::References(referrer))
        .await
        .unwrap()
        .iter()
        .map(|summary| summary.id)
        .collect();
    assert_eq!(references, order, "references in the block's order");

    harness.stop().await;
}
