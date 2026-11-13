use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_member_who_loses_view_when_a_block_is_trashed_is_told_it_is_gone() {
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
    guest.follow(note).await;

    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, note, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    loop {
        let notification = guest.graph_change().await;
        match notification {
            Notice::BlockRemoved { block, .. } if block == note => break,
            Notice::BlockChanged { block, .. } if block.id == note => assert!(
                !block.access.can_view(),
                "the guest may not view the trashed note: {block:?}"
            ),
            _ => {}
        }
    }
    harness.stop().await;
}
