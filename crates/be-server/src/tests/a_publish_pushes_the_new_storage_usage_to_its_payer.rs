use super::*;

#[tokio::test]
async fn a_publish_pushes_the_new_storage_usage_to_its_payer() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("pusher@example.com").await;
    client.organization("pushes").await;
    let author = Author::new(account);
    let mut usage = client::account::storage_changed(&client.peer);
    let first = usage.recv().await.expect("the stream opens with the usage");
    assert_eq!((first.held, first.pending), (0, 0));

    let block = client.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"something worth keeping\n", 1_000, None);
    let published = client
        .publish(&author, block, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let held: u64 = author
        .objects_of(commit, &chunks)
        .iter()
        .map(|hash| author.store.get(*hash).unwrap().unwrap().len() as u64)
        .sum();
    loop {
        let pushed = usage.recv().await.expect("the stream stays open");
        if pushed.held == held {
            assert_eq!(pushed.pending, 0, "a published upload is no longer pending");
            break;
        }
    }

    let mut again = client::account::storage_changed(&client.peer);
    assert_eq!(
        again.recv().await.map(|usage| usage.held),
        Some(held),
        "a second stream opens with the usage too"
    );
    assert!(
        usage.recv().await.is_none(),
        "a connection keeps one storage stream: opening another ends the first"
    );

    let mut stranger = harness.client().await;
    let mut refused = client::account::storage_changed(&stranger.peer);
    assert!(
        refused.recv().await.is_none(),
        "a connection that has not signed in is refused"
    );
    stranger.ended().await;

    let mut unopened = harness.client().await;
    unopened.register("unopened@example.com").await;
    let mut refused = client::account::storage_changed(&unopened.peer);
    assert!(
        refused.recv().await.is_none(),
        "an account with no organization open is refused, not left waiting"
    );
    unopened.round_trip().await;
    harness.stop().await;
}
