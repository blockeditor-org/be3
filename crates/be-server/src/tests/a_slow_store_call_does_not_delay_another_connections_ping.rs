use super::*;

#[tokio::test]
async fn a_slow_store_call_does_not_delay_another_connections_ping() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("slow").await;
    let block = owner.create_block(BlockParent::Root).await;
    let mut other = harness.member("other@example.com").await;

    let (started, release) = crate::store::held::hold(organization);
    let reading = read_block(&owner.peer, block);
    started.await.expect("the read reached the store");
    // The read's job holds the database on a blocking thread until released; a job on the
    // runtime's one thread would keep this ping from ever being answered.
    other.round_trip().await;
    release.send(()).unwrap();
    let read = reading.await.expect("the connection stayed open");
    assert_eq!(read.map(|summary| summary.id), Ok(block));

    harness.stop().await;
}
