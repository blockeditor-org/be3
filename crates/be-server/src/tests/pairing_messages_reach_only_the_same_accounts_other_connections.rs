use super::*;

#[tokio::test]
async fn pairing_messages_reach_only_the_same_accounts_other_connections() {
    let harness = Harness::start().await;
    let mut laptop = harness.client().await;
    laptop.register("owner@example.com").await;
    laptop.organization("notes").await;
    let mut phone = harness.client().await;
    let response = phone
        .call(|peer| client::account::authenticate(peer, laptop.token.clone()))
        .await;
    assert!(matches!(response, Ok(_)));
    let mut stranger = harness.member("stranger@example.com").await;

    let unheard = stranger
        .call(|peer| client::account::pair(peer, None, b"let me in".to_vec()))
        .await;
    assert!(matches!(unheard, Ok(())), "{unheard:?}");

    let response = phone
        .call(|peer| client::account::pair(peer, None, b"request".to_vec()))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let Notice::Paired { from, payload } = laptop.notification().await else {
        panic!("the laptop never heard the request");
    };
    assert_eq!(
        payload.as_slice(),
        b"request".as_slice(),
        "the laptop heard the stranger's request"
    );

    let response = laptop
        .call(|peer| client::account::pair(peer, Some(from), b"reply".to_vec()))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let reply = phone.notification().await;
    assert!(
        matches!(&reply, Notice::Paired { payload, .. } if payload == b"reply"),
        "{reply:?}"
    );

    let response = stranger
        .call(|peer| client::organizations::list_organizations(peer))
        .await;
    assert!(matches!(response, Ok(_)));
    assert!(
        stranger.notifications.is_empty(),
        "another account heard the pairing: {:?}",
        stranger.notifications
    );

    harness.stop().await;
}
