use super::*;

/// A commit's time may run a little past the server's clock, as a fast device's does, but not
/// far: a far-future time would win every comparison by commit time and escape history's
/// thinning by age.
#[tokio::test]
async fn a_commit_from_the_far_future_is_refused() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("future@example.com").await;
    client.organization("future").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let far = be_block_commit::now_milliseconds() + 2 * be_block_commit::retention::HOUR;
    let (forged, chunks) = author.compose(b"from tomorrow\n", far, None);
    let refused = client
        .publish(&author, block, forged, chunks, None, far)
        .await;
    assert!(
        refused_as(&refused, ErrorCode::InvalidRequest),
        "{refused:?}"
    );

    let soon = be_block_commit::now_milliseconds() + be_block_commit::retention::MINUTE;
    let (fast, chunks) = author.compose(b"from a fast clock\n", soon, None);
    let published = client
        .publish(&author, block, fast, chunks, None, soon)
        .await;
    assert!(published.is_ok(), "{published:?}");
    harness.stop().await;
}
