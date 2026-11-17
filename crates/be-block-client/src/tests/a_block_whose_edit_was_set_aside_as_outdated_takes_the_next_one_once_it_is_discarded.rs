use super::*;

async fn grant_carol(revoked: &Revoked, access: Access) {
    revoked
        .admin
        .grant(revoked.team, revoked.carol.account(), access)
        .await
        .unwrap();
    revoked.admin.tend().await.unwrap();
}

/// An edit set aside as sealed under an outdated key, its author demoted by the time it is
/// sent so that it is not sealed again, holds back what is made of its block
/// after it, as an edit the server refuses does, even once its author may write again, until it
/// is discarded: then the block takes the next edit, under a new epoch of the team Bob never
/// gets, and its head uploads as before.
#[tokio::test]
async fn a_block_whose_edit_was_set_aside_as_outdated_takes_the_next_one_once_it_is_discarded() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let made_up = plant(&harness, &revoked, None);
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&revoked.carol, &refs, &store).await;
    assert_eq!(text(&online, revoked.note).await, "before");
    drop(online);
    let offline = harness.offline(&revoked.carol, &refs, &store);
    let saved = offline
        .save(revoked.note, &TextBlock::of("offline"), revoked.head)
        .await;
    assert!(matches!(saved, Ok(Saved::Kept(_))), "{saved:?}");
    drop(offline);
    grant_carol(&revoked, Access::View).await;
    let back = harness.device(&revoked.carol, &refs, &store).await;
    back.drain(|_| false).await.unwrap();
    assert!(back.is_unsent(revoked.note));
    grant_carol(&revoked, Access::Edit).await;

    // Made while it waits: it waits with it.
    let waiting = back
        .save(revoked.note, &TextBlock::of("while it waits"), revoked.head)
        .await;
    assert!(matches!(waiting, Ok(Saved::Kept(_))), "{waiting:?}");
    back.drain(|_| false).await.unwrap();
    assert_eq!(text(&revoked.admin, revoked.note).await, "before");

    assert!(back.discard_unsent(revoked.note).unwrap());
    let head = back.summary(revoked.note).await.unwrap().head;
    assert_eq!(head, revoked.head);
    back.hold(
        revoked.note,
        &[head.unwrap().hash()],
        be_block_protocol::Hold::Lasting,
    )
    .await
    .unwrap();
    let next = back
        .save(revoked.note, &TextBlock::of("next"), head)
        .await
        .unwrap();
    assert!(matches!(next, Saved::Published(_)), "{next:?}");
    assert_eq!(text(&revoked.admin, revoked.note).await, "next");
    let written = back.sealing_key(revoked.note).await.unwrap();
    assert_ne!(written, made_up);
    revoked.bob.refresh_keys(None).await.unwrap();
    assert!(!revoked.bob.keys().has(written));

    harness.stop().await;
}
