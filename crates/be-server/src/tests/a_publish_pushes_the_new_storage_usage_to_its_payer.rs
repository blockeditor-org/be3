use super::*;

#[tokio::test]
async fn a_publish_pushes_the_new_storage_usage_to_its_payer() {
    let harness = Harness::start().await;
    let mut client = harness.member("pusher@example.com").await;
    let account = client.id();
    client.organization("pushes").await;
    let author = Author::new(account);
    let first = client
        .call(|peer| peer.ask::<be_block_protocol::AccountStorage>(Query::Storage, true))
        .await
        .expect("following the storage failed");
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
        client.heard_changed(Topic::Storage).await;
        let usage = client.storage().await.expect("reading the storage failed");
        if usage.held == held {
            assert_eq!(usage.pending, 0, "a published upload is no longer pending");
            break;
        }
    }

    let (second, more) = author.compose(b"and a little more\n", 2_000, None);
    let other = client.create_block(BlockParent::Root).await;
    let published = client
        .publish(&author, other, second, more, None, 2_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    // The topic stays followed, and tells of the upload, then the publish.
    loop {
        client.heard_changed(Topic::Storage).await;
        let usage = client.storage().await.expect("reading the storage failed");
        if usage.held > held {
            break;
        }
    }

    let mut unopened = harness.member("unopened@example.com").await;
    let usage = unopened
        .call(|peer| peer.ask::<be_block_protocol::AccountStorage>(Query::Storage, true))
        .await;
    assert!(
        matches!(&usage, Ok(usage) if usage.held == 0),
        "an account's storage is its own, read in no organization: {usage:?}"
    );
    harness.stop().await;
}
