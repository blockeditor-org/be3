use super::*;

#[tokio::test]
async fn objects_a_block_holds_outlive_its_commits_until_it_is_collected() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    client.organization("history").await;
    let author = Author::new(account);

    let repository = client.create_block(BlockParent::Root).await;
    let (head, chunks) = author.compose(b"a snapshot nobody publishes\n", 1_000, None);
    let objects = author.objects_of(head, &chunks);
    client.upload(&author.store, &objects).await;

    let response = client
        .call(|peer| client::objects::hold_objects(peer, repository, objects.clone()))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let response = client
        .call(|peer| client::objects::hold_objects(peer, repository, objects.clone()))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let response = client
        .call(|peer| {
            client::objects::hold_objects(peer, repository, vec![Hash::of(b"never uploaded")])
        })
        .await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::ObjectNotFound),
        "{response:?}"
    );

    let _ = client
        .call(|peer| client::blocks::delete_block(peer, repository))
        .await;
    let response = client
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    let Ok(be_protocol::Collected { objects: freed, .. }) = response else {
        panic!("collection failed: {response:?}");
    };
    assert_eq!(
        freed,
        objects.len() as u64,
        "holding an object twice counted it twice"
    );
    let response = client
        .call(|peer| client::objects::get_object(peer, chunks[0]))
        .await;
    assert!(matches!(response, Ok(None)), "{response:?}");

    harness.stop().await;
}
