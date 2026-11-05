use super::*;

use crate::objects::tests::TestBucket;

#[tokio::test]
async fn blocks_publish_and_read_back_from_a_bucket() {
    let bucket = TestBucket::start();
    let harness = Harness::configured(ServerConfig {
        bucket: Some(bucket.config(1 << 20)),
        ..ServerConfig::OPEN
    })
    .await;
    let mut client = harness.client().await;
    let account = client.register("writer@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (head, chunks) = author.compose(b"kept in a bucket\n", 1_000, None);
    let objects = author.objects_of(head, &chunks);
    let missing = client
        .call(|peer| client::objects::missing_objects(peer, objects.clone()))
        .await;
    assert!(
        matches!(&missing, Ok(missing) if *missing == objects),
        "{missing:?}"
    );
    let response = client
        .publish(&author, block, head, chunks.clone(), None, 1_000)
        .await;
    assert!(
        matches!(response, Ok(be_block_protocol::PublishOutcome::Published(published)) if published == head),
        "{response:?}"
    );
    let missing = client
        .call(|peer| client::objects::missing_objects(peer, objects.clone()))
        .await;
    assert!(
        matches!(&missing, Ok(missing) if missing.is_empty()),
        "{missing:?}"
    );

    for hash in &objects {
        assert!(
            bucket.file(*hash).exists(),
            "{hash} was not written to the bucket"
        );
    }
    let beside = std::fs::read_dir(harness.directory.join("objects"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(beside, 0, "objects were written beside the database too");

    // With the cache emptied, every object is read back from the bucket.
    std::fs::remove_dir_all(harness.directory.join("object-cache")).unwrap();
    assert_eq!(client.read_block(block).await.head, Some(head));
    for chunk in &chunks {
        let read = client
            .call(|peer| client::objects::get_object(peer, *chunk))
            .await;
        assert!(
            matches!(&read, Ok(Some(bytes)) if Hash::of(bytes) == *chunk),
            "{read:?}"
        );
    }
    let contents = client
        .call(|peer| client::blocks::read_contents(peer, vec![block], Vec::new()))
        .await;
    assert!(
        matches!(&contents, Ok(contents) if !contents.objects.is_empty()),
        "{contents:?}"
    );
    assert!(harness.directory.join("object-cache").exists());

    harness.stop().await;
}
