use super::*;

use be_block_commit::retention::{DAY, MINUTE};
use be_block_graph::SEARCH_INDEX_TYPE;

/// Publishes a commit at each age, oldest first, and answers them in that order.
async fn publish_aged(
    client: &mut TestClient,
    author: &Author,
    block: Uuid,
    now: i64,
    ages: &[i64],
) -> Vec<CommitId> {
    let mut head = None;
    let mut commits = Vec::new();
    for (step, age) in ages.iter().enumerate() {
        let time = now - age;
        let (commit, chunks) = author.compose(format!("state {step}\n").as_bytes(), time, head);
        let published = client
            .publish(author, block, commit, chunks, head, time)
            .await;
        assert!(published.is_ok(), "{published:?}");
        commits.push(commit);
        head = Some(commit);
    }
    commits
}

#[tokio::test]
async fn a_type_with_a_short_retention_keeps_only_its_recent_states() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("indexer@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let workspace = client.create_block(BlockParent::Root).await;
    let index = Uuid::new_v4();
    let key = Uuid::new_v4();
    client
        .create_typed(
            index,
            SEARCH_INDEX_TYPE,
            BlockParent::Block(workspace),
            key,
            KeySet {
                keys: vec![row(key, index, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        )
        .await;
    let note = client.create_block(BlockParent::Root).await;

    let now = be_block_commit::now_milliseconds();
    let ages = [3 * DAY, 2 * DAY, 45 * MINUTE, 25 * MINUTE, 0];
    let indexed = publish_aged(&mut client, &author, index, now, &ages).await;
    let noted = publish_aged(&mut client, &author, note, now, &ages).await;

    let kept = |entries: Vec<HistoryEntry>| {
        let mut kept: Vec<CommitId> = entries.into_iter().map(|entry| entry.commit).collect();
        kept.sort();
        kept
    };
    let sorted = |mut commits: Vec<CommitId>| {
        commits.sort();
        commits
    };
    assert_eq!(
        kept(client.history(index).await),
        sorted(indexed[2..].to_vec()),
        "the index keeps only its states of the last day"
    );
    assert_eq!(
        kept(client.history(note).await),
        sorted(noted),
        "a block of another type keeps the organization's states, one per slot"
    );
    harness.stop().await;
}
