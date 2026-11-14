use super::*;

/// A block's own listing is registered before the store reads it, and what it holds is recorded
/// only once the read returns: a head published in between, which moves nothing but the head,
/// still reaches the reader.
#[tokio::test]
async fn a_head_published_while_its_block_is_being_read_reaches_the_reader() {
    let harness = Harness::start().await;
    let mut publisher = harness.client().await;
    let account = publisher.register("author@example.com").await;
    let organization = publisher.organization("notes").await;
    let block = publisher.create_block(BlockParent::Root).await;
    let mut reader = harness
        .second_connection(&publisher.token.clone(), organization)
        .await;

    let (reached, release) = crate::store::held::hold_listing(block);
    let reading = read_block(&reader.peer, block);
    reached
        .await
        .expect("the read reached the store and was answered");

    let author = Author::new(account);
    let (head, chunks) = author.compose(b"published during the read\n", 1_000, None);
    let published = publisher
        .publish(&author, block, head, chunks, None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");

    release.send(()).unwrap();
    let read = reading.await.expect("the connection stayed open");
    assert!(read.is_ok(), "{read:?}");
    let from = reader.heard_head(block, head).await;
    assert_eq!(Some(from), publisher.client);

    harness.stop().await;
}
