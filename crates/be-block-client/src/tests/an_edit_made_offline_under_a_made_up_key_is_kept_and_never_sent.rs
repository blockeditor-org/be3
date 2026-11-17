use super::*;

/// A device offline, whose read of the team's log fails, judges the epoch by what it kept of
/// the log when it was last online, which holds the revocation, and cannot give the team a new
/// epoch. A person's edit made offline is never refused: it is sealed and kept here. But it is
/// judged again before it is sent, and set aside rather than sent under the key Bob made up,
/// nor uploaded by any other path (signed access 3d).
#[tokio::test]
async fn an_edit_made_offline_under_a_made_up_key_is_kept_and_never_sent() {
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
    assert!(
        offline.keys().trusted(made_up),
        "the made-up key did not open, so the test shows nothing"
    );
    // A part goes up at once, judged by nothing later: refused rather than sealed under the
    // made-up key, as on a device online whose rotation does not get through.
    let part = offline.put_part(revoked.note, b"a part").await;
    assert!(
        matches!(part, Err(ClientError::Locked(_))),
        "a part was sealed under an outdated key: {part:?}"
    );
    let saved = offline
        .save(revoked.note, &TextBlock::of("offline"), revoked.head)
        .await;
    let Ok(Saved::Kept(kept)) = saved else {
        panic!("an edit made offline was refused: {saved:?}");
    };
    assert_eq!(offline.pending().operations, 1);
    drop(offline);

    let back = harness.device(&revoked.carol, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.iter().any(
            |outcome| matches!(outcome, Drained::SetAside { block, .. } if *block == revoked.note)
        ),
        "the edit sealed under the made-up key was not set aside: {drained:?}"
    );
    assert_eq!(
        revoked.admin.summary(revoked.note).await.unwrap().head,
        revoked.head,
        "the edit sealed under the made-up key was sent"
    );
    // Set aside with its reason, its content kept here to restore or discard.
    let unsent = back.unsent();
    let aside = unsent
        .iter()
        .find(|unsent| unsent.block == revoked.note)
        .expect("the edit set aside is not among the unsent");
    assert_eq!(aside.refused, Some(crate::Aside::KeyOutdated));
    assert!(aside.commits.contains(&kept));
    // Nor does any other path upload it: a repository snapshot or a fork holds what it reaches
    // through `hold`, as here.
    let held = back
        .hold(
            revoked.note,
            &[kept.hash()],
            be_block_protocol::Hold::Lasting,
        )
        .await;
    assert!(
        matches!(held, Err(ClientError::Outdated(_))),
        "an edit set aside as outdated was uploaded: {held:?}"
    );

    harness.stop().await;
}
