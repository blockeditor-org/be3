use super::*;

/// A device offline, whose read of the team's log fails, judges the epoch by what it kept of
/// the log when it was last online, which holds the revocation, and cannot give the team a new
/// epoch. A person's edit made offline is never refused: it is sealed and kept here, under the
/// key Bob made up. Judged again before it is sent, it is sealed again under a new epoch of the
/// team, which Bob never gets, and sent; what was sealed under the made-up key never leaves the
/// device (signed access 7).
#[tokio::test]
async fn an_edit_made_offline_under_a_made_up_key_is_sealed_again_under_a_key_bob_never_gets() {
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
    // A second edit on the first: both are sealed again, the second on the first's new commit.
    let saved = offline
        .save(revoked.note, &TextBlock::of("offline, twice"), Some(kept))
        .await;
    let Ok(Saved::Kept(second)) = saved else {
        panic!("an edit made offline was refused: {saved:?}");
    };
    assert_eq!(offline.pending().operations, 2);
    assert_eq!(
        offline.commits().vault().sealed_under(kept.hash()).unwrap(),
        made_up,
        "the edit was not sealed under the made-up key, so the test shows nothing"
    );
    drop(offline);

    let back = harness.device(&revoked.carol, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed {
            block: revoked.note
        }),
        "the edit sealed under the made-up key was not sealed again: {drained:?}"
    );
    let Some(Drained::Published { head, .. }) = drained
        .iter()
        .find(|outcome| matches!(outcome, Drained::Published { block, .. } if *block == revoked.note))
        .copied()
    else {
        panic!("the edit sealed again was not sent: {drained:?}");
    };
    assert_eq!(back.pending().operations, 0);
    assert!(back.unsent().is_empty());
    assert_eq!(
        revoked.admin.summary(revoked.note).await.unwrap().head,
        Some(head)
    );
    assert_ne!(head, second);
    assert_eq!(back.resealed_as(head), vec![second]);
    // Under a key of a new epoch, the content with it, which Bob never gets.
    let written = back.commits().vault().sealed_under(head.hash()).unwrap();
    assert_ne!(written, made_up);
    let manifest = back.commits().get(head).unwrap().manifest;
    for object in back.objects_of(&manifest).await.unwrap() {
        assert_eq!(
            back.commits().vault().sealed_under(object).unwrap(),
            written
        );
    }
    let first = back.commits().get(head).unwrap().parents;
    assert_eq!(first.len(), 1);
    assert_ne!(first[0], kept);
    assert_eq!(back.commits().vault().sealed_under(first[0].hash()).unwrap(), written);
    revoked.bob.refresh_keys(None).await.unwrap();
    assert!(!revoked.bob.keys().has(written));
    revoked.admin.refresh_keys(None).await.unwrap();
    assert_eq!(text(&revoked.admin, revoked.note).await, "offline, twice");
    // What was sealed under the made-up key never leaves this device by any other path: a
    // repository snapshot or a fork holds what it reaches through `hold`, as here.
    let held = back
        .hold(
            revoked.note,
            &[kept.hash()],
            be_block_protocol::Hold::Lasting,
        )
        .await;
    assert!(
        matches!(held, Err(ClientError::Outdated(_))),
        "an edit sealed under the made-up key was uploaded: {held:?}"
    );

    harness.stop().await;
}
