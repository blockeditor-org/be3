use super::*;

#[tokio::test]
async fn a_reserved_object_counts_only_once_its_bytes_arrive() {
    let harness = Harness::start().await;
    let mut client = harness.member("uploader@example.com").await;
    let account = client.id();
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
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::HoldObjects {
                block,
                objects: hashes.clone(),
                hold: Hold::Lasting,
            })
        })
        .await;
    assert!(refused_as(&held, ErrorCode::ObjectNotFound), "{held:?}");
    let published = client
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block,
                expected: None,
                commit: Some(be_block_protocol::NewCommit {
                    id: commit,
                    parents: Vec::new(),
                    held: objects_change(&[], chunks.clone()),
                    time: 1_000,
                    pinned: false,
                    objects: Vec::new(),
                    log_through: None,
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
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
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block,
                expected: None,
                commit: Some(be_block_protocol::NewCommit {
                    id: commit,
                    parents: Vec::new(),
                    held: objects_change(&[], chunks.clone()),
                    time: 1_000,
                    pinned: false,
                    objects: Vec::new(),
                    log_through: None,
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
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
