use super::*;

#[tokio::test]
async fn shared_chunks_survive_until_the_last_commit_releases_them() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let body: String = (0..400).map(|line| format!("line {line}\n")).collect();
    let (first, first_chunks) = author.compose(body.as_bytes(), 1_000, None);
    let appended = format!("{body}one more line\n");
    let (second, second_chunks) = author.compose(appended.as_bytes(), 2_000, Some(first));

    let shared: Vec<_> = first_chunks
        .iter()
        .copied()
        .filter(|chunk| second_chunks.contains(chunk))
        .collect();
    assert!(!shared.is_empty(), "the two drafts shared no chunks");

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
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    client
        .upload(&author.store, &author.objects_of(second, &second_chunks))
        .await;
    let _ = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                second,
                Some(first),
                Some(first).into_iter().collect(),
                second_chunks.clone(),
                2_000,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;

    let response = client
        .call(|peer| client::blocks::prune_history(peer, block, vec![first]))
        .await;
    let Ok(objects) = response else {
        panic!("pruning failed: {response:?}");
    };
    assert!(objects > 0, "pruning the first draft freed nothing");

    for chunk in &shared {
        let response = client
            .call(|peer| client::objects::get_object(peer, *chunk))
            .await;
        assert!(
            matches!(response, Ok(Some(_))),
            "a chunk the surviving commit still holds was deleted"
        );
    }

    let response = client
        .call(|peer| client::objects::get_object(peer, first.hash()))
        .await;
    assert!(
        matches!(response, Ok(None)),
        "the pruned commit object survived"
    );

    assert_eq!(client.history(block).await.len(), 1);
    assert_eq!(client.read_block(block).await.head, Some(second));

    harness.stop().await;
}
