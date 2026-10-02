use super::*;

#[tokio::test]
async fn pairing_messages_reach_only_the_same_accounts_other_connections() {
    let harness = Harness::start().await;
    let mut laptop = harness.client().await;
    laptop.register("owner@example.com").await;
    let workspace = laptop.workspace("notes").await;
    let mut phone = harness.client().await;
    let response = phone
        .send(|request| ClientMessage::Authenticate {
            request,
            version: be_protocol::PROTOCOL_VERSION,
            token: laptop.token.clone(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Authenticated { .. }));
    let mut stranger = harness.member("stranger@example.com").await;

    let refused = stranger
        .send(|request| ClientMessage::Pair {
            request,
            to: None,
            workspace,
            payload: b"let me in".to_vec(),
        })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "a non-member asked to pair: {refused:?}"
    );

    let response = phone
        .send(|request| ClientMessage::Pair {
            request,
            to: None,
            workspace,
            payload: b"request".to_vec(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let ServerMessage::Paired {
        from,
        workspace: asked,
        payload,
    } = laptop.notification().await
    else {
        panic!("the laptop never heard the request");
    };
    assert_eq!(
        (asked, payload.as_slice()),
        (workspace, b"request".as_slice())
    );

    let response = laptop
        .send(|request| ClientMessage::Pair {
            request,
            to: Some(from),
            workspace,
            payload: b"reply".to_vec(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let reply = phone.notification().await;
    assert!(
        matches!(&reply, ServerMessage::Paired { payload, .. } if payload == b"reply"),
        "{reply:?}"
    );

    let response = stranger
        .send(|request| ClientMessage::ListWorkspaces { request })
        .await;
    assert!(matches!(response, ServerMessage::Workspaces { .. }));
    assert!(
        stranger.notifications.is_empty(),
        "another account heard the pairing: {:?}",
        stranger.notifications
    );

    harness.stop().await;
}
