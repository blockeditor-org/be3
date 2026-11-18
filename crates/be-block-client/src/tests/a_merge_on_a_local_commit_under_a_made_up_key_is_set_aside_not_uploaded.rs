use super::*;

/// Offline, Carol makes a commit under the key Bob made up, withdraws it from the outbox, and
/// merges it into the head: the merge waits in the outbox, the commit it merges only in her
/// store. Sealing the outbox again reseals the merge alone, which still holds that commit: it
/// is never uploaded, and the merge is set aside rather than sent.
#[tokio::test]
async fn a_merge_on_a_local_commit_under_a_made_up_key_is_set_aside_not_uploaded() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let made_up = plant(&harness, &revoked, None);
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&revoked.carol, &refs, &store).await;
    assert_eq!(text(&online, revoked.note).await, "before");
    assert!(online.keys().trusted(made_up));
    drop(online);

    let offline = harness.offline(&revoked.carol, &refs, &store);
    let saved = offline
        .save(revoked.note, &TextBlock::of("ours"), revoked.head)
        .await;
    let Ok(Saved::Kept(ours)) = saved else {
        panic!("an edit made offline was refused: {saved:?}");
    };
    assert_eq!(
        offline.commits().vault().sealed_under(ours.hash()).unwrap(),
        made_up,
        "the edit was not sealed under the made-up key, so the test shows nothing"
    );
    offline.withdraw(revoked.note, ours, revoked.head).unwrap();
    assert_eq!(offline.pending().operations, 0);
    let content = TextBlock::of("merged");
    let write = Write::encoded(TextContent::CONTENT_TYPE, content.parts(), revoked.head)
        .merging(revoked.head, ours);
    let merged = offline.write(revoked.note, write).await;
    let Ok(Saved::Kept(merge)) = merged else {
        panic!("a merge made offline was refused: {merged:?}");
    };
    assert_eq!(offline.pending_commits(revoked.note), vec![merge]);
    drop(offline);

    let back = harness.device(&revoked.carol, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block: revoked.note,
            why: crate::Aside::KeyOutdated,
        }),
        "the merge holding a commit under the made-up key was not set aside: {drained:?}"
    );
    assert_eq!(
        revoked.admin.summary(revoked.note).await.unwrap().head,
        revoked.head,
        "the merge holding a commit under the made-up key was sent"
    );
    let held = back
        .hold(
            revoked.note,
            &[ours.hash()],
            be_block_protocol::Hold::Lasting,
        )
        .await;
    assert!(
        matches!(held, Err(ClientError::Outdated(_))),
        "a commit under the made-up key was uploaded: {held:?}"
    );
    assert!(back.is_unsent(revoked.note));

    harness.stop().await;
}
