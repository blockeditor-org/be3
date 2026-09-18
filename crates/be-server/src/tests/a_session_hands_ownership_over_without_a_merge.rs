use super::*;

#[tokio::test]
async fn a_session_hands_ownership_over_without_a_merge() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    let account = phone.register("phone@example.com").await;
    let workspace = phone.workspace("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);

    let (_, state) = phone.join_session(block).await;
    let owner = state.owner.expect("the first participant owns the session");
    assert!(state.is_owner(owner));

    let first = author.compose(b"typed on the phone\n", 1_000, None);
    phone
        .publish(&author, block, first.0, first.1, None, 1_000)
        .await;
    let response = phone
        .send(|request| ClientMessage::Heartbeat {
            request,
            block,
            generation: state.generation,
            clean_at: Some(first.0),
        })
        .await;
    let ServerMessage::Session { state: beating, .. } = response else {
        panic!("heartbeat failed: {response:?}");
    };
    assert_eq!(beating.clean_at, Some(first.0));

    let token = phone.token.clone();
    drop(phone);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut laptop = harness.second_connection(&token, workspace).await;
    let (_, taken) = laptop.join_session(block).await;
    assert!(
        taken.owner.is_some() && taken.owner != Some(owner),
        "the session kept a disconnected owner: {taken:?}"
    );
    assert_eq!(taken.clean_at, Some(first.0));

    let head = laptop.read_block(block).await.head;
    assert!(
        !takeover_needs_merge(taken.clean_at, head),
        "a session that was durable at the head demanded a merge on takeover"
    );

    let second = author.compose(b"continued on the laptop\n", 2_000, Some(first.0));
    let response = laptop
        .publish(&author, block, second.0, second.1, Some(first.0), 2_000)
        .await;
    assert!(
        matches!(response, ServerMessage::Published { .. }),
        "the new owner could not continue from the head: {response:?}"
    );

    let mut returned = harness.second_connection(&token, workspace).await;
    let remote = returned.read_block(block).await.head;
    assert_eq!(
        resume(&author.commits, Some(first.0), remote).unwrap(),
        Resume::FastForward { to: second.0 },
        "the phone should catch up rather than merge"
    );

    harness.stop().await;
}
