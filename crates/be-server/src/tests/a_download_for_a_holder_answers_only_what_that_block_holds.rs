use super::*;

/// Whether the reader gets a URL for `hash` asked for as `holder`'s.
async fn served_as(reader: &mut TestClient, hash: Hash, holder: Uuid) -> bool {
    let urls = reader
        .call(|peer| {
            peer.read::<Vec<Option<String>>>(be_block_protocol::Query::DownloadUrls {
                objects: vec![hash],
                holder: Some(holder),
            })
        })
        .await;
    let Ok([url]) = urls.as_deref() else {
        panic!("one answer for one object: {urls:?}");
    };
    url.is_some()
}

/// `download-urls` with a `holder` answers an object only as that block's: one the block holds
/// and the reader may view. Another block the reader cannot view holding it, or a block the
/// reader views that does not hold it, gives no URL, though the same object asked for with no
/// holder is served.
#[tokio::test]
async fn a_download_for_a_holder_answers_only_what_that_block_holds() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let author = Author::new(owner.id());
    let organization = owner.organization("shared").await;
    let viewed = owner.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"held by two blocks\n", 1_000, None);
    let published = owner
        .publish(&author, viewed, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(
        matches!(
            published,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{published:?}"
    );
    let hidden = owner.create_block(BlockParent::Root).await;
    let held = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::HoldObjects {
                block: hidden,
                objects: vec![chunks[0]],
                hold: Hold::Lasting,
            })
        })
        .await;
    assert!(matches!(held, Ok(())), "{held:?}");
    let empty = owner.create_block(BlockParent::Root).await;

    let mut reader = harness.member("reader@example.com").await;
    let invited = owner
        .invite_with(Uuid::from_u128(1), organization, Vec::new(), Vec::new())
        .await;
    assert!(matches!(invited, Ok(())), "{invited:?}");
    let _ = reader.accept(Uuid::from_u128(1), &Vec::new()).await;
    reader.open(organization).await;
    for block in [viewed, empty] {
        let granted = owner
            .grant(block, reader.id(), be_block_graph::Access::View)
            .await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }

    assert!(
        reader.download(chunks[0]).await.unwrap().is_some(),
        "with no holder, a block the reader views holds it"
    );
    assert!(served_as(&mut reader, chunks[0], viewed).await);
    assert!(
        !served_as(&mut reader, chunks[0], hidden).await,
        "a holder the reader cannot view"
    );
    assert!(
        !served_as(&mut reader, chunks[0], empty).await,
        "a block the reader views that does not hold it"
    );

    harness.stop().await;
}
