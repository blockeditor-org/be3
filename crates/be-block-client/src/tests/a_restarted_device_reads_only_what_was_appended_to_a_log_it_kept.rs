use super::*;

use be_block_graph::Access;
use be_type_text::TextContent;

/// A device keeps a log it checked whole with its refs, entries and all: after a restart its
/// first read names the last entry it kept, is sent only what was appended since, and checks and
/// hashes only that, not the whole log again.
#[tokio::test]
async fn a_restarted_device_reads_only_what_was_appended_to_a_log_it_kept() {
    const GRANTS: usize = 60;
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let viewer = harness.member(&owner, "viewer@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let device = harness.device(&owner, &refs, &store).await;
    let block = device
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    for at in 0..GRANTS {
        let level = match at % 2 {
            0 => Access::View,
            _ => Access::Edit,
        };
        device.grant(block, viewer.account(), level).await.unwrap();
    }
    let (entries, _) = device.checked_access(block).await.unwrap();
    assert!(entries.len() > GRANTS);
    drop(device);

    // Another device of the account appends one more.
    owner.refresh_keys(None).await.unwrap();
    owner
        .grant(block, viewer.account(), Access::View)
        .await
        .unwrap();

    let restarted = harness.device(&owner, &refs, &store).await;
    let (hashed, verified) = (
        be_block_keys::entries_hashed(),
        be_block_keys::signatures_verified(),
    );
    let (entries, checked) = restarted.checked_access(block).await.unwrap();
    let hashed = be_block_keys::entries_hashed() - hashed;
    let verified = be_block_keys::signatures_verified() - verified;
    assert_eq!(
        Some(checked.head),
        entries.last().map(be_block_keys::Step::hash),
        "the log is checked to the appended entry"
    );
    assert!(
        verified <= 2,
        "only the appended entry is verified: {verified}"
    );
    assert!(
        hashed < (GRANTS / 4) as u64,
        "and the kept entries are not hashed again: {hashed} of {}",
        entries.len()
    );

    harness.stop().await;
}
