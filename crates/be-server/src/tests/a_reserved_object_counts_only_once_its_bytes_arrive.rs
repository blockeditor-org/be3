use super::*;

#[tokio::test]
async fn a_reserved_object_counts_only_once_its_bytes_arrive() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("uploader@example.com").await;
    client.organization("uploads").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"sent after its reservation\n", 1_000, None);
    let hashes = author.objects_of(commit, &chunks);
    let objects: Vec<Vec<u8>> = hashes
        .iter()
        .map(|hash| author.store.get(*hash).unwrap().unwrap())
        .collect();

    let urls: Vec<UploadUrl> = client
        .reserve(&objects)
        .await
        .unwrap()
        .into_iter()
        .map(|url| url.expect("an object not sent yet is asked for"))
        .collect();
    let lifetime: u64 = url::Url::parse(&urls[0].url)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "X-Amz-Expires")
        .map(|(_, seconds)| seconds.parse().unwrap())
        .unwrap();
    assert!(
        Duration::from_secs(lifetime) < crate::store::DEFAULT_UPLOAD_GRACE / 2,
        "an upload URL outlives half its reservation's grace: {lifetime}s"
    );

    // Reserved but not sent: the server holds none of it.
    let missing = client.missing(&objects).await;
    assert_eq!(missing.as_ref().ok(), Some(&hashes), "{missing:?}");
    let held = client
        .call(|peer| client::objects::hold_objects(peer, block, hashes.clone(), Hold::Lasting))
        .await;
    assert!(refused_as(&held, ErrorCode::ObjectNotFound), "{held:?}");
    let published = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
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
    assert!(
        refused_as(&published, ErrorCode::ObjectNotFound),
        "{published:?}"
    );
    assert_eq!(client.download(commit.hash()).await, Ok(None));

    // Once every object is sent, it is the uploader's to publish and then to read.
    for (url, bytes) in urls.into_iter().zip(objects.clone()) {
        assert_eq!(put_to(url, bytes).await, 200);
    }
    let missing = client.missing(&objects).await;
    assert_eq!(missing, Ok(Vec::new()));
    let published = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
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
    assert!(
        matches!(
            published,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{published:?}"
    );
    let read = client.download(chunks[0]).await;
    assert!(
        matches!(&read, Ok(Some(bytes)) if Hash::of(bytes) == chunks[0]),
        "{read:?}"
    );

    harness.stop().await;
}
