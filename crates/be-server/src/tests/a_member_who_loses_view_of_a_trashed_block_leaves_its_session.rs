use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_member_who_loses_view_of_a_trashed_block_leaves_its_session() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;

    let shared = owner.create_block(BlockParent::Root).await;
    let note = owner.create_block(BlockParent::Block(shared)).await;
    let granted = owner.grant(shared, guest_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    let (author, _) = owner.join_session(note).await;
    let (visitor, state) = guest.join_session(note).await;
    assert!(state.participants.contains(&visitor));

    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, note, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let left = loop {
        match owner.notification().await {
            Notice::SessionChanged { block, state }
                if block == note && !state.participants.contains(&visitor) =>
            {
                break state;
            }
            _ => {}
        }
    };
    assert!(left.participants.contains(&author));
    harness.stop().await;
}
