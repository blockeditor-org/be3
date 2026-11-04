use super::the_trees_owner_pays_for_what_anyone_makes_in_it::{SharedTree, held, shared_tree};
use super::*;

#[tokio::test]
async fn a_move_past_the_new_payers_quota_is_refused() {
    const QUOTA: u64 = 4 * 1024;
    let harness = Harness::configured(ServerConfig {
        account_quota: Some(QUOTA),
        ..ServerConfig::OPEN
    })
    .await;
    let SharedTree {
        mut owner,
        mut editor,
        child,
        size,
    } = shared_tree(&harness).await;
    let filler = vec![9; usize::try_from(QUOTA - size / 2).unwrap()];
    let stored = editor
        .call(|peer| client::objects::put_objects(peer, vec![filler]))
        .await;
    assert!(stored.is_ok(), "{stored:?}");

    let moved = owner.move_to(child, BlockParent::Root).await;
    assert!(
        refused_as(&moved, ErrorCode::OverQuota),
        "a move charged its block to an account with no room: {moved:?}"
    );
    assert_eq!(
        held(&mut owner).await,
        size,
        "a refused move left the charge"
    );
    assert_eq!(held(&mut editor).await, 0);
    assert_ne!(
        owner.read_block(child).await.parent,
        Some(BlockParent::Root),
        "a refused move left the block where it was"
    );

    harness.stop().await;
}
