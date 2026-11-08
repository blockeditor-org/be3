use super::*;

#[tokio::test]
async fn block_sizes_add_up_their_subtrees_and_follow_moves() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("sizer@example.com").await;
    let organization = client.organization("sizes").await;
    let author = Author::new(account);
    let bytes_of = |hashes: &[Hash]| -> u64 {
        hashes
            .iter()
            .map(|hash| author.store.get(*hash).unwrap().unwrap().len() as u64)
            .sum()
    };

    let folder = client.create_block(BlockParent::Root).await;
    let child = client.create_block(BlockParent::Block(folder)).await;
    let elsewhere = client.create_block(BlockParent::Root).await;

    let text = b"a line that both versions keep\n".repeat(64);
    let (first, first_chunks) = author.compose(&text, minutes_ago(2), None);
    let published = client
        .publish(
            &author,
            child,
            first,
            first_chunks.clone(),
            None,
            minutes_ago(2),
        )
        .await;
    assert!(published.is_ok(), "{published:?}");
    let mut longer = text.clone();
    longer.extend_from_slice(b"and one more line\n");
    let (second, second_chunks) = author.compose(&longer, minutes_ago(1), Some(first));
    let published = client
        .publish(
            &author,
            child,
            second,
            second_chunks.clone(),
            Some(first),
            minutes_ago(1),
        )
        .await;
    assert!(published.is_ok(), "{published:?}");

    let mut distinct: Vec<Hash> = author.objects_of(first, &first_chunks);
    distinct.extend(author.objects_of(second, &second_chunks));
    distinct.sort_unstable();
    distinct.dedup();
    let size = bytes_of(&distinct);
    assert!(
        size < bytes_of(&author.objects_of(first, &first_chunks))
            + bytes_of(&author.objects_of(second, &second_chunks)),
        "the two versions share a chunk"
    );

    let summary = client.read_block(child).await;
    assert_eq!(
        (summary.size, summary.tree_size),
        (size, size),
        "a block counts every version it keeps, each object once"
    );
    let summary = client.read_block(folder).await;
    assert_eq!((summary.size, summary.tree_size), (0, size));

    let usage = client
        .call(|peer| client::account::storage_usage(peer))
        .await
        .unwrap();
    assert_eq!(usage.held, size, "the author pays for what its block holds");
    assert_eq!(usage.pending, 0, "a published upload is no longer pending");
    assert_eq!(usage.quota, Some(crate::store::DEFAULT_ACCOUNT_QUOTA));

    let token = client.token.clone();
    let moved = client
        .call(|peer| {
            client::blocks::set_parent(
                peer,
                child,
                BlockParent::Block(elsewhere),
                KeySet::default(),
            )
        })
        .await;
    assert!(moved.is_ok(), "{moved:?}");
    assert_eq!(client.read_block(folder).await.tree_size, 0);
    assert_eq!(client.read_block(elsewhere).await.tree_size, size);

    let harness = harness.restart().await;
    let mut client = harness.second_connection(&token, organization).await;
    assert_eq!(
        client.read_block(elsewhere).await.tree_size,
        size,
        "sizes outlive a restart"
    );

    let deleted = client
        .call(|peer| client::blocks::delete_block(peer, child))
        .await;
    assert!(deleted.is_ok(), "{deleted:?}");
    assert_eq!(
        client.read_block(elsewhere).await.tree_size,
        0,
        "a deleted block left its parent"
    );
    let collected = client
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    assert!(collected.is_ok(), "{collected:?}");
    let usage = client
        .call(|peer| client::account::storage_usage(peer))
        .await
        .unwrap();
    assert_eq!(usage.held, 0, "collecting a block stops charging for it");

    harness.stop().await;
}
