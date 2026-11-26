use super::*;

use be_block_graph::Access;
use be_type_text::TextContent;

/// How many signatures `device` verifies joining `world`'s host, `host`.
async fn joins(device: Peer<MemoryStore>, world: Uuid, host: Uuid) -> u64 {
    device.refresh_contacts().await.unwrap();
    let before = be_block_keys::signatures_verified();
    device.join_host(world, host).await.unwrap();
    be_block_keys::signatures_verified() - before
}

/// A Join member, holding no key that pins the block's root, checks the log a host is named in
/// from the root a contact's grant vouches for once, and takes what it checked: not again
/// through the root it then pins. Served a proof path, which is checked whole each time, it
/// checks no more on its first join than on a later one.
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
    let host = owner.account();
    let first = joins(harness.device(&player, &refs, &store).await, world, host).await;
    let again = joins(harness.device(&player, &refs, &store).await, world, host).await;
    assert!(again > 0, "a proof path is checked whole each time");
    assert_eq!(
        first, again,
        "the first join checks the log once, not again once it pins its root"
    );

    harness.stop().await;
}
