use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_member_who_loses_view_when_a_block_moves_somewhere_hidden_is_told_it_is_gone() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;

    let shared = owner.create_block(BlockParent::Root).await;
    let note = owner.create_block(BlockParent::Block(shared)).await;
    let hidden = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(shared, guest_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    guest.follow(note).await;

    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, note, BlockParent::Block(hidden), KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    loop {
        let notification = guest.graph_change().await;
        match notification {
            Notice::BlockRemoved { block, .. } if block == note => break,
            Notice::BlockChanged { block, .. } if block.id == note => assert!(
                !block.access.can_view(),
                "the guest may not view the note in the hidden folder: {block:?}"
            ),
            _ => {}
        }
    }

    // Back where the guest may view it, its next head reaches the guest again: the note's own
    // listing outlived the view.
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
