use super::*;
use be_block_graph::Access;
use be_block_session::Membership;

#[tokio::test]
async fn a_member_who_loses_view_of_a_trashed_block_leaves_its_session() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
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
    let (author, mut seen) = owner.join_session(note).await;
    let (visitor, state) = guest.join_session(note).await;
    assert!(state.has(visitor));

    let response = owner
        .move_keyed(note, BlockParent::Detached, KeySet::default())
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    while seen.has(visitor) || seen.index < 2 {
        let index = seen.index + 1;
        owner.membership_until(note, &mut seen, index).await;
    }
    assert!(seen.has(author));
    harness.stop().await;
}
