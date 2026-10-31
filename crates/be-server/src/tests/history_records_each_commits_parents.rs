use super::*;

#[tokio::test]
async fn history_records_each_commits_parents() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("historian@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (first, first_chunks) = author.compose(b"a first draft\n", 1_000, None);
    let (second, second_chunks) = author.compose(b"a second draft\n", 2_000, Some(first));
    let _ = client
        .publish(&author, block, first, first_chunks, None, 1_000)
        .await;
    let _ = client
        .publish(&author, block, second, second_chunks, Some(first), 2_000)
        .await;

    let history: Vec<_> = client
        .history(block)
        .await
        .into_iter()
        .map(|entry| (entry.commit, entry.parents))
        .collect();
    assert_eq!(history, [(second, vec![first]), (first, Vec::new())]);

    harness.stop().await;
}
