use super::*;

/// Entries appended offline go in one append as the device drains, and the server refuses an
/// append whole: one entry it refuses (over the size limit) among good ones would refuse them
/// all. The batch goes again an entry at a time, so only the bad one is kept as refused and the
/// rest are logged in order.
#[tokio::test]
async fn one_bad_entry_among_good_ones_is_the_only_one_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let chat = workspace(&owner, "Chat").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&owner, &refs, &store).await;
    online
        .append(chat, "test.app", b"zero", be_block_content::Along::default())
        .await
        .unwrap();
    drop(online);
    let laptop = harness.offline(&owner, &refs, &store);
    let append = |text: &'static str| {
        let laptop = &laptop;
        async move {
            match laptop
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
                appended => panic!("an entry made offline was not kept: {appended:?}"),
            }
        }
    };
    let first = append("one").await;
    // What a device checks before keeping it is the limit the server holds it to: an entry
    // over it reaches the outbox only as a device the server's limit has since changed for.
    let bad = Uuid::new_v4();
    laptop.keep_in_outbox(refs::Op::Append(refs::Appending {
            block: chat,
            id: bad,
            payload: vec![0; be_block_protocol::MAX_LOG_ENTRY_BYTES + 1],
            signature: vec![0; 64],
            announces: None,
            redacts: None,
        }));
    let last = append("two").await;

    drop(laptop);
    let laptop = harness.device(&owner, &refs, &store).await;
    let drained = laptop.drain(|_| false).await.unwrap();
    let appended: Vec<Uuid> = drained
        .iter()
        .filter_map(|outcome| match outcome {
            Drained::Appended { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(appended, [first, last], "the good entries are logged, in order");
    let refused: Vec<Uuid> = drained
        .iter()
        .filter_map(|outcome| match outcome {
            Drained::NotAppended { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(refused, [bad], "only the bad entry is refused");
    assert_eq!(laptop.pending().operations, 0);
    harness.stop().await;
}
