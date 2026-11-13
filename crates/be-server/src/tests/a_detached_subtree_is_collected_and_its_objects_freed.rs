use super::*;

#[tokio::test]
async fn a_detached_subtree_is_collected_and_its_objects_freed() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("gardener@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);

    let folder = client.create_block(BlockParent::Root).await;
    let child = client.create_block(BlockParent::Block(folder)).await;
    let bystander = client.create_block(BlockParent::Root).await;

    let (head, chunks) = author.compose(b"contents of the child\n", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let _ = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                child,
                head,
                None,
                Vec::new(),
                objects_change(&[], chunks.clone()),
                1_000,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;

    let _ = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                bystander,
                head,
                None,
                Vec::new(),
                objects_change(&[], chunks.clone()),
                1_000,
                false,
                listing(&[], vec![child]),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;

    let response = client
        .call(|peer| client::blocks::list_blocks(peer, Listing::Backrefs(child)))
        .await;
    let Ok(be_block_protocol::Listed { blocks, .. }) = response else {
        panic!("backrefs failed: {response:?}");
    };
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id, bystander);

    let response = client
        .call(|peer| {
            client::blocks::set_parent(peer, folder, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let response = client
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    let Ok(be_block_protocol::Collected {
        blocks, objects, ..
    }) = response
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

    let response = client.download(chunks[0]).await;
    assert!(matches!(response, Ok(Some(_))), "{response:?}");

    let response = client
        .try_read(child).await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::BlockNotFound),
        "{response:?}"
    );
    assert!(client.read_block(bystander).await.head.is_some());

    harness.stop().await;
}
