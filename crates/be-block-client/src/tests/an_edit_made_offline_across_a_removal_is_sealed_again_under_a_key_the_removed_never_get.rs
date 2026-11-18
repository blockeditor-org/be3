use super::a_demoted_administrator_with_the_server_reads_nothing_written_after_the_demotion::opened_with_the_server;
use super::*;

use be_block_graph::Access;

/// A device goes offline before a member is removed from a workspace, and edits a note in it
/// under the key at hand, which the removed member holds. Reconnecting, it finds that key
/// outdated by the removal in the workspace's log and seals the edit again under a new epoch,
/// which the removed member, with the server, does not open; what was first sealed never
/// leaves the device.
#[tokio::test]
async fn an_edit_made_offline_across_a_removal_is_sealed_again_under_a_key_the_removed_never_get() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    let public = admin.verified_identity(reader.account()).unwrap();
    admin
        .share(team, reader.account(), Access::Edit, &public)
        .await
        .unwrap();
    assert_eq!(text(&reader, note).await, "before");
    let removed = &reader;

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&admin, &refs, &store).await;
    assert_eq!(text(&online, note).await, "before");
    drop(online);

    admin
        .grant(team, reader.account(), Access::None)
        .await
        .unwrap();
    admin.tend().await.unwrap();

    let offline = harness.offline(&admin, &refs, &store);
    let saved = offline.save(note, &TextBlock::of("offline"), head).await;
    let Ok(Saved::Kept(kept)) = saved else {
        panic!("an edit made offline was refused: {saved:?}");
    };
    let first = offline.commits().vault().sealed_under(kept.hash()).unwrap();
    assert!(
        opened_with_the_server(&harness, removed).contains(&first.to_string()),
        "the edit was not sealed under a key the removed member holds, so the test shows nothing"
    );
    drop(offline);

    let back = harness.device(&admin, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed { block: note }),
        "the edit sealed under a key from before the removal was not sealed again: {drained:?}"
    );
    let Some(Drained::Published { head, .. }) = drained
        .iter()
        .find(|outcome| matches!(outcome, Drained::Published { block, .. } if *block == note))
        .copied()
    else {
        panic!("the edit sealed again was not sent: {drained:?}");
    };
    let written = back.commits().vault().sealed_under(head.hash()).unwrap();
    assert_ne!(written, first);
    assert!(
        !opened_with_the_server(&harness, removed).contains(&written.to_string()),
        "the removed member, with the server, opens the edit sealed again"
    );
    assert_eq!(text(&harness.second(&admin).await, note).await, "offline");

    harness.stop().await;
}
