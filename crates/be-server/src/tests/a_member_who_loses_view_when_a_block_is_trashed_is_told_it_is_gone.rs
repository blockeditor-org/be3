use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_member_who_loses_view_when_a_block_is_trashed_is_told_it_is_gone() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let mut guest = harness.member("guest@example.com").await;
    let guest_id = guest.id();
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;

    let shared = owner.create_block(BlockParent::Root).await;
    let note = owner.create_block(BlockParent::Block(shared)).await;
    let granted = owner.grant(shared, guest_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    guest.follow(note).await;

    let response = owner
        .move_keyed(note, BlockParent::Detached, KeySet::default())
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(
        guest.heard_left(note).await,
        shared,
        "the shared folder's reader is told the note left it"
    );

    // Back where the guest may view it, its next head reaches the guest again: the guest still
    // follows the shared folder.
    let restored = owner.move_to(note, BlockParent::Block(shared)).await;
    assert!(matches!(restored, Ok(())), "{restored:?}");
    let author = Author::new(owner_id);
    let (head, chunks) = author.compose(b"written once it was back\n", 1_000, None);
    let published = owner
        .publish(&author, note, head, chunks, None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    guest.heard_head(note, head).await;
    harness.stop().await;
}
