use super::*;

use be_block::block_url::block_url;
use be_graph::Access;
use be_protocol::{BlockSummary, KeyKind};
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_block_a_shared_one_links_to_shows_only_that_it_exists() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let link = |block| block_url("https://example.com/", owner.organization(), block);
    let further = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    owner
        .save(further, &TextBlock::of("two links away"), None)
        .await
        .unwrap();
    let linked = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let linked_head = owner
        .save(
            linked,
            &TextBlock::of(&format!("Private plan, see {}", link(further))),
            None,
        )
        .await
        .unwrap()
        .published()
        .unwrap();
    let shared = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    owner
        .save(
            shared,
            &TextBlock::of(&format!("Notes, see {}", link(linked))),
            None,
        )
        .await
        .unwrap();
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(shared, guest.account(), Access::View, &public)
        .await
        .unwrap();

    let listed = guest.shared().await.unwrap();
    guest.learn(&listed).await.unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|summary| (summary.id, summary.access))
            .collect::<Vec<_>>(),
        [(shared, Access::View)]
    );
    assert_eq!(
        guest
            .summary(linked)
            .await
            .ok()
            .map(|summary| BlockSummary {
                revision: 0,
                ..summary
            }),
        Some(BlockSummary::no_access(
            linked,
            TextContent::CONTENT_TYPE,
            Access::KnowExists
        )),
        "a linked block the guest cannot view tells it its id, type and access and nothing else"
    );
    assert!(
        guest.summary(further).await.is_err(),
        "knowing a block exists does not reveal what that block links to"
    );
    assert!(guest.summary(mine).await.is_err());

    guest.refresh_keys(Some(vec![linked])).await.unwrap();
    assert!(guest.keys().keys_of(linked, KeyKind::Data).is_empty());
    assert!(guest.keys().keys_of(linked, KeyKind::Scope).is_empty());
    assert!(guest.open::<TextContent>(linked).await.is_err());
    assert!(
        guest.load_commit(linked_head).await.is_err(),
        "the guest fetched an object of a block it may only know exists"
    );
    assert_eq!(
        text(&guest, shared).await,
        format!("Notes, see {}", link(linked))
    );

    harness.stop().await;
}
