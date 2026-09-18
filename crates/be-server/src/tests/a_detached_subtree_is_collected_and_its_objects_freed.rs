use super::*;

#[tokio::test]
async fn a_detached_subtree_is_collected_and_its_objects_freed() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("gardener@example.com").await;
    client.workspace("notes").await;
    let author = Author::new(account);

    let folder = client.create_block(BlockParent::Root).await;
    let child = client.create_block(BlockParent::Block(folder)).await;
    let bystander = client.create_block(BlockParent::Root).await;

    let (head, chunks) = author.compose(b"contents of the child\n", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    client
        .send(|request| ClientMessage::Publish {
            request,
            block: child,
            commit: head,
            expected: None,
            chunks: chunks.clone(),
            time: 1_000,
            pinned: false,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;

    client
        .send(|request| ClientMessage::Publish {
            request,
            block: bystander,
            commit: head,
            expected: None,
            chunks: chunks.clone(),
            time: 1_000,
            pinned: false,
            references_added: vec![child],
            references_removed: Vec::new(),
        })
        .await;

    let response = client
        .send(|request| ClientMessage::ListBackrefs {
            request,
            block: child,
        })
        .await;
    let ServerMessage::Blocks { blocks, .. } = response else {
        panic!("backrefs failed: {response:?}");
    };
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id, bystander);

    let response = client
        .send(|request| ClientMessage::DeleteBlock {
            request,
            block: folder,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let response = client
        .send(|request| ClientMessage::CollectDetached { request })
        .await;
    let ServerMessage::Collected {
        blocks, objects, ..
    } = response
    else {
        panic!("collection failed: {response:?}");
    };
    assert_eq!(blocks.len(), 2, "a referenced child kept its subtree alive");
    assert!(blocks.contains(&folder));
    assert!(blocks.contains(&child));
    assert_eq!(
        objects, 0,
        "an object another commit still holds was deleted"
    );

    let response = client
        .send(|request| ClientMessage::GetObject {
            request,
            hash: chunks[0],
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Object { bytes: Some(_), .. }),
        "{response:?}"
    );

    let response = client
        .send(|request| ClientMessage::ReadBlock {
            request,
            block: child,
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::BlockNotFound),
        "{response:?}"
    );
    assert!(client.read_block(bystander).await.head.is_some());

    harness.stop().await;
}
