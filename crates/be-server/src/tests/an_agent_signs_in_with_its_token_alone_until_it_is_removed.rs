use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use super::*;

#[tokio::test]
async fn an_agent_signs_in_with_its_token_alone_until_it_is_removed() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let workspace = owner.workspace("tracker").await;

    let response = owner
        .send(|request| ClientMessage::CreateAgent {
            request,
            workspace,
            name: "Claude".into(),
            recovery: [7; 32],
            sealed: vec![1, 2, 3],
        })
        .await;
    let ServerMessage::AgentCreated { agent, token, .. } = response else {
        panic!("creating an agent failed: {response:?}");
    };
    assert_eq!(agent.name, "Claude");

    let mut request = harness.url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (socket, _) = connect_async(request).await.unwrap();
    let mut bot = TestClient {
        socket,
        next: 0,
        notifications: Vec::new(),
        token: String::new(),
    };
    let response = bot.send(|request| ClientMessage::WhoAmI { request }).await;
    let ServerMessage::Authenticated {
        account,
        token: echoed,
        ..
    } = response
    else {
        panic!("the header did not sign the agent in: {response:?}");
    };
    assert_eq!(account, agent.account);
    assert!(echoed.is_empty(), "the server must not echo a header token");
    bot.open(workspace).await;
    let response = bot.send(|request| ClientMessage::GetKeys { request }).await;
    let ServerMessage::Keys {
        recovery, sealed, ..
    } = response
    else {
        panic!("reading keys failed: {response:?}");
    };
    assert_eq!(recovery, Some([7; 32]));
    assert_eq!(sealed.len(), 1);
    assert_eq!(sealed[0].sealed, [1, 2, 3]);

    let response = owner
        .send(|request| ClientMessage::ListAgents { request, workspace })
        .await;
    let ServerMessage::Agents { agents, .. } = response else {
        panic!("listing agents failed: {response:?}");
    };
    assert_eq!(agents, [agent.clone()]);

    let response = owner
        .send(|request| ClientMessage::RemoveAgent {
            request,
            workspace,
            account: agent.account,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let mut again = harness.client().await;
    let response = again
        .send(|request| ClientMessage::Authenticate { request, token })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { .. }),
        "a removed agent's token still works: {response:?}"
    );

    harness.stop().await;
}
