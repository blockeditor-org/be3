use super::*;

#[tokio::test]
async fn a_batch_answers_each_request_in_order() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    client.register("batcher@example.com").await;
    client.organization("notes").await;
    let folder = client.create_block(BlockParent::Root).await;
    let missing = Uuid::new_v4();

    let response = client
        .send(|request| ClientMessage::Batch {
            request,
            messages: vec![
                ClientMessage::ReadBlock {
                    request: 1,
                    block: missing,
                },
                ClientMessage::Batch {
                    request: 2,
                    messages: Vec::new(),
                },
                ClientMessage::ReadBlock {
                    request: 3,
                    block: folder,
                },
            ],
        })
        .await;
    let ServerMessage::Batched { responses, .. } = response else {
        panic!("a batch is answered as one: {response:?}");
    };
    assert!(
        matches!(responses.as_slice(), [
            ServerMessage::Failed { request: 1, .. },
            ServerMessage::Failed { request: 2, code: ErrorCode::InvalidRequest, .. },
            ServerMessage::Block { request: 3, block, .. },
        ] if block.id == folder),
        "each request is answered in order, a failure stops nothing after it, and a batch holds no batch: {responses:?}"
    );
}
