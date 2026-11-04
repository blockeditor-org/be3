use super::*;

#[tokio::test]
async fn pruning_history_keeps_pinned_commits() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (first, first_chunks) = author.compose(b"a bookmarked draft\n", 1_000, None);
    let (second, second_chunks) = author.compose(b"a later draft\n", 2_000, Some(first));
    client
        .upload(&author.store, &author.objects_of(first, &first_chunks))
        .await;
    let _ = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                first,
                None,
                Vec::new(),
                first_chunks.clone(),
                1_000,
                true,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    let _ = client
        .publish(&author, block, second, second_chunks, Some(first), 2_000)
        .await;

    let response = client
        .call(|peer| client::blocks::prune_history(peer, block, vec![first, second]))
        .await;
    let Ok(objects) = response else {
        panic!("pruning failed: {response:?}");
    };
    assert_eq!(objects, 0, "pruning freed a pinned commit or the head");

    let kept: Vec<CommitId> = client
        .history(block)
        .await
        .into_iter()
        .map(|entry| entry.commit)
        .collect();
    assert!(kept.contains(&first), "the pinned commit was pruned");
    assert!(kept.contains(&second), "the head was pruned");

    let response = client
        .call(|peer| client::objects::get_object(peer, first.hash()))
        .await;
    assert!(
        matches!(response, Ok(Some(_))),
        "the pinned commit's object was deleted"
    );

    harness.stop().await;
}
