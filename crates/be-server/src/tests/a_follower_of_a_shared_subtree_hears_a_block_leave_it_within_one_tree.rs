use super::*;
use be_block_graph::Access;

/// A guest the owner made a contact of, in the owner's personal organization.
async fn guest(harness: &Harness, owner: &mut TestClient, email: &str) -> (TestClient, Uuid) {
    let mut guest = harness.client().await;
    let account = guest.register(email).await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;
    (guest, account)
}

#[tokio::test]
async fn a_follower_of_a_shared_subtree_hears_a_block_leave_it_within_one_tree() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let (mut first, first_id) = guest(&harness, &mut owner, "first@example.com").await;
    let (mut second, second_id) = guest(&harness, &mut owner, "second@example.com").await;

    let top = owner.create_block(BlockParent::Root).await;
    let shared = owner.create_block(BlockParent::Block(top)).await;
    let sibling = owner.create_block(BlockParent::Block(top)).await;
    let note = owner.create_block(BlockParent::Block(shared)).await;
    let granted = owner.grant(shared, first_id, Access::View).await;
    assert!(granted.is_ok(), "{granted:?}");
    let granted = owner.grant(sibling, second_id, Access::View).await;
    assert!(granted.is_ok(), "{granted:?}");
    assert_eq!(
        first.follow(note).await,
        shared,
        "a block viewed below a hidden parent is followed through where the view starts"
    );
    assert_eq!(second.follow(sibling).await, sibling);

    // The note moves to the top of the same tree: out of the shared subtree, and into neither.
    let moved = owner.move_to(note, BlockParent::Block(top)).await;
    assert!(moved.is_ok(), "{moved:?}");
    assert_eq!(first.heard_left(note).await, shared);

    // The sibling's follower hears of the next change in its own tree, and of nothing before.
    let renamed = owner
        .call(|peer| client::blocks::set_metadata(peer, sibling, b"renamed".to_vec(), Vec::new()))
        .await;
    assert!(renamed.is_ok(), "{renamed:?}");
    let entry = loop {
        if let Notice::TreeAdvanced { entry, .. } = second.graph_change().await {
            break entry;
        }
    };
    assert_eq!(entry, sibling);
    let (changed, left) = second.read_changes(sibling).await;
    assert_eq!(
        changed.iter().map(|block| block.id).collect::<Vec<_>>(),
        [sibling]
    );
    assert!(left.is_empty(), "{left:?}");

    harness.stop().await;
}
