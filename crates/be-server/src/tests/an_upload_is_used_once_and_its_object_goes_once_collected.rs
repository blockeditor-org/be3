use super::*;

#[tokio::test]
async fn an_upload_is_used_once_and_its_object_goes_once_collected() {
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
        metadata_database(&harness.directory)
            .query_row("SELECT COUNT(*) FROM object_uploads", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(uploads(), 0, "publishing used the uploads it held");
    let usage = client.storage().await.unwrap();
    assert_eq!(usage.pending, 0);

    // Asked again for what the block holds, the server reserves nothing and asks for none of it.
    let bytes: Vec<Vec<u8>> = objects
        .iter()
        .map(|hash| author.store.get(*hash).unwrap().unwrap())
        .collect();
    let missing = client.missing(&bytes).await.unwrap();
    assert!(missing.is_empty(), "the server holds what the block holds");
    assert_eq!(uploads(), 0, "nothing held is reserved again");

    // Once the last block that held them is collected, so are they, and they are asked for again.
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
    assert_eq!(collected.objects as usize, objects.len());
    let missing = client.missing(&bytes).await.unwrap();
    assert_eq!(missing, objects, "collected objects are asked for again");

    harness.stop().await;
}
