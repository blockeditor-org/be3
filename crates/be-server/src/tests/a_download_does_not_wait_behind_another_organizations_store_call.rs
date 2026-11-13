use super::*;

#[tokio::test]
async fn a_download_does_not_wait_behind_another_organizations_store_call() {
    let harness = Harness::start().await;
    let mut reader = harness.client().await;
    let account = reader.register("reader@example.com").await;
    reader.organization("library").await;
    let author = Author::new(account);
    let block = reader.create_block(BlockParent::Root).await;
    let (commit, chunks) =
        author.compose(b"read while another call holds the database\n", 1_000, None);
    let published = reader
        .publish(&author, block, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(
        matches!(
            published,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{published:?}"
    );

    let mut busy = harness.member("busy@example.com").await;
    let organization = busy.organization("busy").await;
    let held = busy.create_block(BlockParent::Root).await;
    let (started, release) = crate::store::held::hold(organization);
    let waiting = client::blocks::list_blocks(&busy.peer, Listing::Block(held));
    started.await.expect("the call reached the store");

    // The busy organization's call holds the database lock until released; the download
    // reads through the read-only connection and the library's cached graph.
    let downloaded = reader.download(chunks[0]).await;
    assert!(matches!(downloaded, Ok(Some(_))), "{downloaded:?}");

    release.send(()).unwrap();
    let read = waiting.await.expect("the connection stayed open");
    assert_eq!(read.map(|listed| the_block(listed).id), Ok(held));

    harness.stop().await;
}
