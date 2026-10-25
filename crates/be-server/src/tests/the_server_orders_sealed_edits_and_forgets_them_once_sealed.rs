use super::*;

async fn sequenced(client: &mut TestClient) -> be_protocol::SessionOp {
    loop {
        if let ServerMessage::Sequenced { op, .. } = client.notification().await {
            return op;
        }
    }
}

async fn log(client: &mut TestClient, block: Uuid, origin: u64) -> be_protocol::SessionLog {
    let response = client
        .send(|request| ClientMessage::Catchup {
            request,
            block,
            origin,
        })
        .await;
    let ServerMessage::SessionLog { log, .. } = response else {
        panic!("catching up failed: {response:?}");
    };
    log
}

#[tokio::test]
async fn the_server_orders_sealed_edits_and_forgets_them_once_sealed() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    let account = phone.register("phone@example.com").await;
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let first = author.compose(b"base\n", 1_000, None);
    phone
        .publish(&author, block, first.0, first.1, None, 1_000)
        .await;

    let (_, state) = phone.join_session(block).await;
    let mut laptop = harness
        .second_connection(&phone.token.clone(), organization)
        .await;
    laptop.join_session(block).await;

    let plaintext = b"insert \"the quick brown fox\" at 12";
    let sealed = author.commits.vault().seal(KEY, plaintext).unwrap();
    let id = OpId {
        origin: 41,
        counter: 1,
    };
    for _ in 0..2 {
        let response = laptop
            .send(|request| ClientMessage::Submit {
                request,
                block,
                id,
                base: 0,
                barrier: false,
                payload: sealed.clone(),
            })
            .await;
        assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    }
    for client in [&mut phone, &mut laptop] {
        let op = sequenced(client).await;
        assert_eq!((op.id, op.sequence, op.base, op.floor), (id, 1, 0, 0));
        assert_eq!(op.payload, sealed, "the server changed what it ordered");
        assert!(
            !op.payload
                .windows(plaintext.len())
                .any(|window| window == plaintext)
        );
    }

    let held = log(&mut phone, block, 41).await;
    assert_eq!(held.ops.len(), 1, "a resubmitted edit was ordered twice");
    assert_eq!(held.counter, 1);
    assert_eq!(held.seal.head, Some(first.0));
    assert_eq!(held.seal.sequence, 0);

    let mut other = harness
        .second_connection(&phone.token.clone(), organization)
        .await;
    other.join_session_as(block, [9; 32]).await;
    let response = other
        .send(|request| ClientMessage::Submit {
            request,
            block,
            id: OpId {
                origin: 42,
                counter: 1,
            },
            base: 1,
            barrier: false,
            payload: sealed.clone(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::PermissionDenied),
        "another build edited in the session: {response:?}"
    );

    let second = author.compose(b"base\nfox\n", 2_000, Some(first.0));
    phone
        .publish(&author, block, second.0, second.1, Some(first.0), 2_000)
        .await;
    let state_bytes = author.commits.vault().seal(KEY, b"session state").unwrap();
    let response = phone
        .send(|request| ClientMessage::Heartbeat {
            request,
            block,
            generation: state.generation,
            seal: SessionSeal {
                head: Some(second.0),
                sequence: 1,
                state: Some(state_bytes.clone()),
            },
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Session { .. }),
        "{response:?}"
    );
    let trimmed = log(&mut laptop, block, 41).await;
    assert!(trimmed.ops.is_empty(), "the server kept what was sealed");
    assert_eq!(trimmed.seal.head, Some(second.0));
    assert_eq!(trimmed.seal.sequence, 1);
    assert_eq!(trimmed.seal.state, Some(state_bytes));
    assert_eq!(trimmed.counter, 1);

    harness.stop().await;
}
