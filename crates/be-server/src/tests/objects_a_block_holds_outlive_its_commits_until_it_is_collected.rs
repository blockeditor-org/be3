use super::*;

#[tokio::test]
async fn objects_a_block_holds_outlive_its_commits_until_it_is_collected() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    client.workspace("history").await;
    let author = Author::new(account);

    let repository = client.create_block(BlockParent::Root).await;
    let (head, chunks) = author.compose(b"a snapshot nobody publishes\n", 1_000, None);
    let objects = author.objects_of(head, &chunks);
    client.upload(&author.store, &objects).await;

    let response = client
        .send(|request| ClientMessage::HoldObjects {
            request,
            block: repository,
            objects: objects.clone(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let response = client
        .send(|request| ClientMessage::HoldObjects {
            request,
            block: repository,
            objects: objects.clone(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let response = client
        .send(|request| ClientMessage::HoldObjects {
            request,
            block: repository,
            objects: vec![Hash::of(b"never uploaded")],
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::ObjectNotFound),
        "{response:?}"
    );

    client
        .send(|request| ClientMessage::DeleteBlock {
            request,
            block: repository,
        })
        .await;
    let response = client
        .send(|request| ClientMessage::CollectDetached { request })
        .await;
    let ServerMessage::Collected { objects: freed, .. } = response else {
        panic!("collection failed: {response:?}");
    };
    assert_eq!(
        freed,
        objects.len(),
        "holding an object twice counted it twice"
    );
    let response = client
        .send(|request| ClientMessage::GetObject {
            request,
            hash: chunks[0],
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Object { bytes: None, .. }),
        "{response:?}"
    );

    harness.stop().await;
}
