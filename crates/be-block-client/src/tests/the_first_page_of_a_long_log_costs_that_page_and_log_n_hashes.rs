use super::*;

/// A device reading a page of a long log, its first or one a search hit points into, fetches
/// that page and O(log n) hashes that tie it to the log's head, and keeps only that page: it
/// never reads the log from its start to trust a part of it.
#[tokio::test]
async fn the_first_page_of_a_long_log_costs_that_page_and_log_n_hashes() {
    const ENTRIES: u64 = 1 << 16;
    let harness = Harness::start().await;
    let maker = harness.owner("owner@example.com").await;
    let chat = workspace(&maker, "Chat").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let owner = harness.device(&maker, &refs, &store).await;
    let mut database = harness.database();
    let transaction = database.transaction().unwrap();
    for batch in 0..ENTRIES / 4096 {
        let entries = (0..4096)
            .map(|index| be_block_protocol::NewEntry {
                id: Uuid::from_u64_pair(batch, index),
                payload: (batch * 4096 + index).to_be_bytes().to_vec(),
                signature: vec![1; 64],
            })
            .collect();
        be_server::log::append(
            &transaction,
            owner.organization(),
            chat,
            owner.account(),
            entries,
            1_000,
        )
        .unwrap();
    }
    transaction.commit().unwrap();
    drop(database);

    let first = owner.read_log(chat, 0, 50).await.unwrap();
    assert_eq!(first.head.size, ENTRIES);
    assert_eq!(first.entries.len(), 50);
    assert!(
        first.proof_hashes <= 16,
        "the first page took {} hashes",
        first.proof_hashes
    );
    let hit = owner.read_log(chat, ENTRIES / 2 + 7, 50).await.unwrap();
    assert_eq!(hit.entries[0].entry.seq, ENTRIES / 2 + 7);
    assert!(
        hit.proof_hashes <= 2 * 16,
        "a page in the middle took {} hashes",
        hit.proof_hashes
    );
    drop(owner);
    let offline = harness.offline(&maker, &refs, &store);
    let kept_first = offline.read_log(chat, 0, u32::MAX).await.unwrap();
    let kept_hit = offline.read_log(chat, ENTRIES / 2, u32::MAX).await.unwrap();
    assert_eq!(kept_first.head, first.head);
    assert_eq!(
        (kept_first.entries.len(), kept_hit.entries.len()),
        (50, 0),
        "the device kept more than the pages it read"
    );
    assert_eq!(
        offline
            .read_log(chat, ENTRIES / 2 + 7, u32::MAX)
            .await
            .unwrap()
            .entries
            .len(),
        50
    );
    harness.stop().await;
}
