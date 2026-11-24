use super::*;

/// An entry appended offline that the server refuses when the device drains, its author
/// demoted meanwhile, leaves the outbox and is kept, with why, across the device starting
/// again, until it is let go: what shows of it outlives the editor that appended it.
#[tokio::test]
async fn an_entry_the_server_refuses_is_kept_until_it_is_let_go() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&revoked.carol, &refs, &store).await;
    assert_eq!(text(&online, revoked.note).await, "before");
    drop(online);
    let offline = harness.offline(&revoked.carol, &refs, &store);
    let id = match offline
        .append(
            revoked.note,
            "test.app",
            b"written away",
            be_block_content::Along::default(),
        )
        .await
        .unwrap()
    {
        Appended::Kept { id } => id,
        appended => panic!("an entry made offline was not kept: {appended:?}"),
    };
    drop(offline);

    revoked
        .admin
        .grant(revoked.team, revoked.carol.account(), Access::View)
        .await
        .unwrap();
    revoked.admin.tend().await.unwrap();
    let back = harness.device(&revoked.carol, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained.iter().any(|outcome| matches!(
            outcome,
            Drained::NotAppended { block, id: refused, .. }
                if *block == revoked.note && *refused == id
        )),
        "{drained:?}"
    );
    assert_eq!(back.pending().operations, 0, "it is not sent again");
    let refused = back.refused_entries(revoked.note);
    assert_eq!(refused.len(), 1);
    assert_eq!(
        (refused[0].id, &refused[0].payload[..]),
        (id, &b"written away"[..])
    );
    assert!(!refused[0].why.is_empty(), "it says why");
    drop(back);

    let again = harness.offline(&revoked.carol, &refs, &store);
    assert_eq!(
        again
            .refused_entries(revoked.note)
            .into_iter()
            .map(|refused| refused.id)
            .collect::<Vec<_>>(),
        [id],
        "the device keeps it across starting again"
    );
    again.let_go_refused(revoked.note, id).unwrap();
    drop(again);
    let after = harness.offline(&revoked.carol, &refs, &store);
    assert!(after.refused_entries(revoked.note).is_empty());
    harness.stop().await;
}
