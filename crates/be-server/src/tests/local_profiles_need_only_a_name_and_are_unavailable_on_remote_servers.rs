use super::*;

#[tokio::test]
async fn local_profiles_need_only_a_name_and_are_unavailable_on_remote_servers() {
    let remote = Harness::start().await;
    let mut client = remote.client().await;
    assert!(matches!(
        client
            .send(|request| ClientMessage::CreateLocalProfile {
                request,
                display_name: "Ada".into()
            })
            .await,
        ServerMessage::Failed { .. }
    ));
    let local = Harness::configured(ServerConfig {
        local_profiles: true,
        ..ServerConfig::default()
    })
    .await;
    let mut client = local.client().await;
    assert!(matches!(
        client
            .send(|request| ClientMessage::CreateLocalProfile {
                request,
                display_name: " ".into()
            })
            .await,
        ServerMessage::Failed { .. }
    ));
    let created = client
        .send(|request| ClientMessage::CreateLocalProfile {
            request,
            display_name: " Ada ".into(),
        })
        .await;
    let ServerMessage::Authenticated {
        account,
        display_name,
        email,
        token,
        ..
    } = created
    else {
        panic!("{created:?}")
    };
    assert_eq!(display_name, "Ada");
    assert!(email.is_empty());
    let resumed = client
        .send(|request| ClientMessage::Authenticate { request, token })
        .await;
    assert!(
        matches!(resumed, ServerMessage::Authenticated { account: id, email, .. } if id == account && email.is_empty())
    );
    assert!(
        matches!(client.send(|request| ClientMessage::CreateLocalProfile { request, display_name: "Ada".into() }).await, ServerMessage::Authenticated { account: id, .. } if id != account)
    );
    assert_eq!(
        http(
            &local,
            "POST",
            "/api/be/http",
            "",
            Some("https://unrelated.example")
        )
        .await
        .0,
        403
    );
}
