use super::*;

/// A publish with no commit sets a block's metadata and references while its head is
/// `expected`, which stays its head; one expecting another head is rejected with the head, as
/// a stale publish with a commit is.
#[tokio::test]
async fn a_publish_with_no_commit_changes_the_metadata_and_leaves_the_head() {
    let harness = Harness::start().await;
    let mut connection = harness.member("author@example.com").await;
    let account = connection.id();
    connection.personal().await;
    let block = connection.create_block(BlockParent::Root).await;
    let linked = connection.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"first\n", 1_000, None);
    let published = connection
        .publish(&author, block, head, chunks, None, 1_000)
        .await;
    assert!(
        matches!(published, Ok(be_block_protocol::PublishOutcome::Published(_))),
        "{published:?}"
    );

    let describe = |expected| Op::Publish {
        block,
        expected,
        commit: None,
        references: listing(&[], vec![linked]),
        metadata: b"described".to_vec(),
        journal: None,
    };
    let stale = connection
        .call(|peer| peer.write::<be_block_protocol::PublishOutcome>(describe(None)))
        .await;
    assert_eq!(
        stale,
        Ok(be_block_protocol::PublishOutcome::Rejected(Some(head)))
    );
    let described = connection
        .call(|peer| peer.write::<BlockSummary>(describe(Some(head))))
        .await
        .expect("a publish with no commit at the head was refused");
    assert_eq!(
        (described.head, described.metadata.as_slice()),
        (Some(head), &b"described"[..])
    );
    let read = connection.read_block(block).await;
    assert_eq!(
        (read.head, read.metadata, read.references),
        (Some(head), b"described".to_vec(), vec![linked])
    );

    harness.stop().await;
}
