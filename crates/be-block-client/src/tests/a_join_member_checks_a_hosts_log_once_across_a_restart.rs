use super::*;

use be_block_graph::Access;
use be_type_text::TextContent;

/// A Join member, holding no key that pins the block's root, checks the log a host is named in
/// from the root a contact's grant vouches for, once, and keeps what it checked: joined again
/// after a restart, it checks only the entries after what it kept, not the whole log again.
#[tokio::test]
async fn a_join_member_checks_a_hosts_log_once_across_a_restart() {
    const GRANTS: usize = 40;
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let viewer = harness.member(&owner, "viewer@example.com").await;
    let player = harness.member(&owner, "player@example.com").await;
    player.refresh_contacts().await.unwrap();
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    owner
        .grant(world, player.account(), Access::Join)
        .await
        .unwrap();
    for at in 0..GRANTS {
        let level = match at % 2 {
            0 => Access::View,
            _ => Access::Edit,
        };
        owner.grant(world, viewer.account(), level).await.unwrap();
    }

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let joins = |device: Peer<MemoryStore>| async move {
        device.refresh_contacts().await.unwrap();
        let before = be_block_keys::signatures_verified();
        device.join_host(world, owner.account()).await.unwrap();
        be_block_keys::signatures_verified() - before
    };
    let first = joins(harness.device(&player, &refs, &store).await).await;
    assert!(
        first >= GRANTS as u64,
        "the first join checks the whole log: {first}"
    );
    assert!(
        first < 2 * GRANTS as u64,
        "and checks it once, not again once its root is pinned: {first}"
    );

    let again = joins(harness.device(&player, &refs, &store).await).await;
    assert!(
        again < (GRANTS / 4) as u64,
        "joined after a restart, it checks nothing of what it kept: {again}"
    );

    harness.stop().await;
}
