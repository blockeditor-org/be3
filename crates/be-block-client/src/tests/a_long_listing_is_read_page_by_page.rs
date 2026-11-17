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
    let first = crate::ask(be_block_protocol::read::<be_block_protocol::Listed, _>(
        &author.connection(),
        Some(author.organization()),
        be_block_protocol::Query::List {
            listing: Listing::Children(BlockParent::Block(workspace)),
            after: None,
        },
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
    let referred = crate::ask(
        be_block_protocol::write::<be_block_protocol::BlockSummary, _>(
            &author.connection(),
            Some(author.organization()),
            be_block_protocol::Op::Publish {
                block: referrer,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &order.clone()),
                metadata: Vec::new(),
                journal: None,
            },
        ),
    )
    .await;
    assert!(referred.is_ok(), "{referred:?}");
    let first = crate::ask(be_block_protocol::read::<be_block_protocol::Listed, _>(
        &author.connection(),
        Some(author.organization()),
        be_block_protocol::Query::List {
            listing: Listing::References(referrer),
            after: None,
        },
    ))
    .await
    .unwrap();
    assert!(first.next.is_some(), "the references fit one page");
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
