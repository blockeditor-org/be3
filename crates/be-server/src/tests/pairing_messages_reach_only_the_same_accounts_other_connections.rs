use super::*;

#[tokio::test]
async fn pairing_messages_reach_only_the_same_accounts_other_connections() {
    let harness = Harness::start().await;
    let mut laptop = harness.client().await;
    laptop.register("owner@example.com").await;
    laptop.organization("notes").await;
    let mut phone = harness.client().await;
    let response = phone
        .send(|request| ClientMessage::Authenticate {
            request,
            token: laptop.token.clone(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Authenticated { .. }));
    let mut stranger = harness.member("stranger@example.com").await;

    let unheard = stranger
        .send(|request| ClientMessage::Pair {
            request,
            to: None,
            payload: b"let me in".to_vec(),
        })
        .await;
    assert!(matches!(unheard, ServerMessage::Ok { .. }), "{unheard:?}");

    let response = phone
        .send(|request| ClientMessage::Pair {
            request,
            to: None,
            payload: b"request".to_vec(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let ServerMessage::Paired { from, payload } = laptop.notification().await else {
        panic!("the laptop never heard the request");
    };
    assert_eq!(
        payload.as_slice(),
        b"request".as_slice(),
        "the laptop heard the stranger's request"
    );

    let response = laptop
        .send(|request| ClientMessage::Pair {
            request,
            to: Some(from),
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
        .send(|request| ClientMessage::ListOrganizations { request })
        .await;
    assert!(matches!(response, ServerMessage::Organizations { .. }));
    assert!(
        stranger.notifications.is_empty(),
        "another account heard the pairing: {:?}",
        stranger.notifications
    );

    harness.stop().await;
}
