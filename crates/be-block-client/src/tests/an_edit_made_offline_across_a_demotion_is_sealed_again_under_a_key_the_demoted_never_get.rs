use super::a_demoted_administrator_with_the_server_reads_nothing_written_after_the_demotion::opened_with_the_server;
use super::*;

use be_block_protocol::OrganizationRole;

/// A member's device goes offline before an administrator's demotion, and edits under the
/// epoch at hand, which the demoted administrator reached through escrow: offline it knows of
/// no demotion, so the edit is sealed under that epoch and kept. Reconnecting, it reads the
/// organization's log before sending, finds the epoch outdated by the demotion, and seals the
/// edit again under a new epoch (signed access 7), which the demoted administrator, with the
/// server, does not open; what was first sealed never leaves the device (signed access 8).
#[tokio::test]
async fn an_edit_made_offline_across_a_demotion_is_sealed_again_under_a_key_the_demoted_never_get()
{
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&member, "Team").await;
    let note = member
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = member
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let promoted = harness.second(&deputy).await;
    assert_eq!(text(&promoted, note).await, "before");

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&member, &refs, &store).await;
    assert_eq!(text(&online, note).await, "before");
    drop(online);

    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();
    admin.tend().await.unwrap();

    let offline = harness.offline(&member, &refs, &store);
    let saved = offline.save(note, &TextBlock::of("offline"), head).await;
    let Ok(Saved::Kept(kept)) = saved else {
        panic!("an edit made offline was refused: {saved:?}");
    };
    let first = offline.commits().vault().sealed_under(kept.hash()).unwrap();
    assert!(
        opened_with_the_server(&harness, &promoted).contains(&first.to_string()),
        "the edit was not sealed under an epoch the deputy holds, so the test shows nothing"
    );
    drop(offline);

    let back = harness.device(&member, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed { block: note }),
        "the edit sealed under an epoch from before the demotion was not sealed again: {drained:?}"
    );
    let Some(Drained::Published { head, .. }) = drained
        .iter()
        .find(|outcome| matches!(outcome, Drained::Published { block, .. } if *block == note))
        .copied()
    else {
        panic!("the edit sealed again was not sent: {drained:?}");
    };
    assert!(back.unsent().is_empty());
    let written = back.commits().vault().sealed_under(head.hash()).unwrap();
    assert_ne!(written, first);
    assert!(
        !opened_with_the_server(&harness, &promoted).contains(&written.to_string()),
        "the demoted administrator, with the server, opens the edit sealed again"
    );
    let held = back
        .hold(note, &[kept.hash()], be_block_protocol::Hold::Lasting)
        .await;
    assert!(
        matches!(held, Err(ClientError::Outdated(_))),
        "the edit sealed under the outdated epoch was uploaded: {held:?}"
    );
    assert_eq!(text(&harness.second(&admin).await, note).await, "offline");

    harness.stop().await;
}
