use super::*;

/// Carol edits offline under the key Bob made up, and is demoted to View meanwhile. Back
/// online, the logs show the key outdated and Carol no longer writing the block: her edit is
/// not sealed again under a new epoch, but set aside, never sent.
#[tokio::test]
async fn an_edit_made_offline_by_an_author_demoted_since_stays_set_aside() {
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
        .save(revoked.note, &TextBlock::of("offline"), revoked.head)
        .await;
    let Ok(Saved::Kept(kept)) = saved else {
        panic!("an edit made offline was refused: {saved:?}");
    };
    drop(offline);

    revoked
        .admin
        .grant(revoked.team, revoked.carol.account(), Access::View)
        .await
        .unwrap();
    revoked.admin.tend().await.unwrap();
    let epochs = revoked
        .admin
        .keys()
        .keys_of(revoked.team, KeyKind::Scope)
        .len();

    let back = harness.device(&revoked.carol, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block: revoked.note,
            why: crate::Aside::KeyOutdated,
        }),
        "the edit of an author demoted since was not set aside: {drained:?}"
    );
    assert!(
        !drained.contains(&Drained::Resealed {
            block: revoked.note
        }),
        "the edit of an author demoted since was sealed again: {drained:?}"
    );
    assert_eq!(
        revoked.admin.summary(revoked.note).await.unwrap().head,
        revoked.head,
        "the edit of an author demoted since was sent"
    );
    revoked.admin.refresh_keys(None).await.unwrap();
    assert_eq!(
        revoked
            .admin
            .keys()
            .keys_of(revoked.team, KeyKind::Scope)
            .len(),
        epochs,
        "the demoted author gave the team a new epoch"
    );
    let unsent = back.unsent();
    let aside = unsent
        .iter()
        .find(|unsent| unsent.block == revoked.note)
        .expect("the edit set aside is not among the unsent");
    assert_eq!(aside.refused, Some(crate::Aside::KeyOutdated));
    assert!(aside.commits.contains(&kept));

    harness.stop().await;
}
