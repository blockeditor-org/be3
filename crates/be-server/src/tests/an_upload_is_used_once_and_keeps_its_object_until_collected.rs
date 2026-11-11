use super::*;

#[tokio::test]
async fn an_upload_is_used_once_and_keeps_its_object_until_collected() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("reuser@example.com").await;
    client.organization("uploads").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;
    let (head, chunks) = author.compose(b"held by one block\n", 1_000, None);
    let objects = author.objects_of(head, &chunks);
    let published = client
        .publish(&author, block, head, chunks.clone(), None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let uploads = || {
        rusqlite::Connection::open(harness.directory.join("metadata.sqlite"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM object_uploads", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(uploads(), 0, "publishing used the uploads it held");
    let usage = client
        .call(|peer| client::account::storage_usage(peer))
        .await
        .unwrap();
    assert_eq!(usage.pending, 0);

    client.upload(&author.store, &objects).await;
    assert_eq!(uploads() as usize, objects.len());
    let deleted = client
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(deleted.is_ok(), "{deleted:?}");
    let collected = client
        .call(|peer| client::blocks::collect_detached(peer))
        .await
        .unwrap();
    assert_eq!(
        collected.objects, 0,
        "an object with an unused upload outlives the last block that held it"
    );
    let missing = client
        .call(|peer| client::objects::missing_objects(peer, objects.clone()))
        .await
        .unwrap();
    assert!(missing.is_empty(), "the uploader may still use its upload");

    harness.stop().await;
}
