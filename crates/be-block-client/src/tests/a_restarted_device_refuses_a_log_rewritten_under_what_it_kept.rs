use super::*;

use be_block_graph::Access;
use be_block_keys::Entry;
use be_type_text::TextContent;

/// A device restarted on the log it kept names its root and last entry to read on from them.
/// A server that rewrote the entry it holds, or swapped the root under it, sends the log from
/// its root, and the device refuses it as forged rather than take its kept prefix for it.
#[tokio::test]
async fn a_restarted_device_refuses_a_log_rewritten_under_what_it_kept() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let viewer = harness.member(&owner, "viewer@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let device = harness.device(&owner, &refs, &store).await;
    let block = device
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    for level in [Access::View, Access::Edit, Access::View] {
        device.grant(block, viewer.account(), level).await.unwrap();
    }
    let (entries, _) = device.checked_access(block).await.unwrap();
    drop(device);
    let held = entries.len() - 1;
    let rewrite = |position: usize, entry: &Entry| {
        harness
            .database()
            .execute(
                "UPDATE access_log SET entry = ?1 WHERE block_id = ?2 AND position = ?3",
                rusqlite::params![entry.encode(), block.to_string(), position],
            )
            .unwrap();
    };
    let forged = |previous| {
        Entry::sign(
            stranger.keys().identity(),
            stranger.account().into_bytes(),
            block.into_bytes(),
            previous,
            false,
            be_block_keys::Record::Grant(be_block_keys::Grant {
                grantee: stranger.account().into_bytes(),
                identity: stranger.keys().identity().identity(),
                level: be_block_keys::Level::Manage,
                authority: be_block_keys::Authority::Log,
            }),
        )
    };

    // The entry the device holds, rewritten in place.
    let Some(be_block_keys::Step::Entry(last)) = entries.last() else {
        panic!("the device holds its own log whole");
    };
    rewrite(held, &forged(last.previous));
    let restarted = harness.device(&owner, &refs, &store).await;
    assert!(
        matches!(restarted.checked_access(block).await, Err(ClientError::Forged(at)) if at == block),
        "a log whose held entry was rewritten is refused"
    );
    drop(restarted);
    let be_block_keys::Step::Entry(original) = &entries[held] else {
        unreachable!();
    };
    rewrite(held, original);

    // The root swapped for one the server controls.
    let swapped = Entry::root(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        be_block_keys::LogKind::Block,
        block.into_bytes(),
    );
    rewrite(0, &swapped);
    let restarted = harness.device(&owner, &refs, &store).await;
    assert!(
        matches!(restarted.checked_access(block).await, Err(ClientError::Forged(at)) if at == block),
        "a log whose root was swapped is refused"
    );

    harness.stop().await;
}
