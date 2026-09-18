use super::*;

#[tokio::test]
async fn relayed_session_traffic_passes_through_the_server_sealed() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let workspace = owner.workspace("notes").await;
    let block = owner.create_block(BlockParent::Root).await;
    let author = Author::new(account);

    let (owner_id, _) = owner.join_session(block).await;
    let mut peer = harness
        .second_connection(&owner.token.clone(), workspace)
        .await;
    let (peer_id, state) = peer.join_session(block).await;
    assert_eq!(state.owner, Some(owner_id));
    assert!(state.participants.contains(&peer_id));

    let plaintext = b"insert \"the quick brown fox\" at 12";
    let sealed = author.commits.vault().seal(plaintext);
    assert!(
        !sealed
            .windows(plaintext.len())
            .any(|window| window == plaintext),
        "the relayed payload carried its plaintext"
    );

    let response = peer
        .send(|request| ClientMessage::Relay {
            request,
            block,
            to: Some(owner_id),
            payload: sealed.clone(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let joined = owner.notification().await;
    assert!(
        matches!(joined, ServerMessage::SessionChanged { state: ref changed, .. }
            if changed.participants.contains(&peer_id)),
        "the owner was not told a peer joined: {joined:?}"
    );

    let relayed = owner.notification().await;
    let ServerMessage::Relayed {
        block: on,
        from,
        payload,
    } = relayed
    else {
        panic!("the relay never arrived: {relayed:?}");
    };
    assert_eq!(on, block);
    assert_eq!(from, peer_id);
    assert_eq!(payload, sealed);
    assert_eq!(author.commits.vault().open(&payload).unwrap(), plaintext);

    let response = peer
        .send(|request| ClientMessage::Relay {
            request,
            block,
            to: Some(9_999),
            payload: sealed,
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::InvalidRequest),
        "{response:?}"
    );

    harness.stop().await;
}
