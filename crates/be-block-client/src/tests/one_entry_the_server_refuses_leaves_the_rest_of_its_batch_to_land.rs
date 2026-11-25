use super::*;

/// The server refuses an append whole, so one entry it would refuse, sent in a batch among
/// good ones, refuses the batch; its entries go again one at a time, and only that one is
/// kept as refused while the others land, in the order they were made.
#[tokio::test]
async fn one_entry_the_server_refuses_leaves_the_rest_of_its_batch_to_land() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let chat = workspace(&laptop, "Chat").await;
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let append = async |text: &str| match offline
        .append(
            chat,
            "test.app",
            text.as_bytes(),
            be_block_content::Along::default(),
        )
        .await
        .unwrap()
    {
        Appended::Kept { id } => id,
        logged => panic!("an entry made offline was logged: {logged:?}"),
    };
    let before = append("before").await;
    // Over the server's limit for an entry, which `append` would not have kept.
    let bad = Uuid::new_v4();
    offline.refs.push(refs::Op::Append(refs::Appending {
        block: chat,
        id: bad,
        payload: vec![0; be_block_protocol::MAX_LOG_ENTRY_BYTES + 1],
        signature: vec![0; 64],
        announces: None,
        redacts: None,
    }))
    .unwrap();
    let after = append("after").await;

    harness.reconnect(&offline, owner.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    let appended: Vec<(Uuid, u64)> = drained
        .iter()
        .filter_map(|outcome| match outcome {
            Drained::Appended { block, id, seq, .. } if *block == chat => Some((*id, *seq)),
            _ => None,
        })
        .collect();
    assert_eq!(appended, [(before, 0), (after, 1)], "{drained:?}");
    let refused: Vec<Uuid> = drained
        .iter()
        .filter_map(|outcome| match outcome {
            Drained::NotAppended { block, id, .. } if *block == chat => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(refused, [bad], "{drained:?}");
    assert_eq!(offline.pending(), refs::PendingCounts::default());
    // Kept as refused (its bytes are not sealed, so `refused_entries` would leave it out).
    assert_eq!(
        offline
            .refs
            .not_appended(chat)
            .into_iter()
            .map(|refused| refused.id)
            .collect::<Vec<_>>(),
        [bad]
    );

    let read = offline.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(
        read.entries
            .iter()
            .map(|logged| logged.bytes().unwrap().to_vec())
            .collect::<Vec<_>>(),
        [&b"before"[..], b"after"]
    );
    harness.stop().await;
}
