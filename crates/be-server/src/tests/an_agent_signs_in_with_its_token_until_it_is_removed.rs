use super::*;

#[tokio::test]
async fn an_agent_signs_in_with_its_token_until_it_is_removed() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("tracker").await;

    let response = owner
        .call(|peer| {
            client::members::create_agent(
                peer,
                organization,
                "Claude".into(),
                [7; 32],
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    let Ok(be_protocol::CreatedAgent { agent, token }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    assert_eq!(agent.name, "Claude");

    let mut bot = harness.client().await;
    let response = bot
        .call(|peer| client::account::authenticate(peer, Some(token.clone())))
        .await;
    let Ok(Authenticated { account, .. }) = response else {
        panic!("the agent's token did not sign it in: {response:?}");
    };
    assert_eq!(account, agent.account);
    bot.open(organization).await;
    let response = bot.call(|peer| client::keys::get_recovery_key(peer)).await;
    let Ok(be_protocol::RecoveryKey { public, sealed, .. }) = response else {
        panic!("reading the recovery key failed: {response:?}");
    };
    assert_eq!(public, Some([7; 32]));
    assert!(sealed.is_empty());

    let response = owner
        .call(|peer| client::members::list_agents(peer, organization))
        .await;
    let Ok(agents) = response else {
        panic!("listing agents failed: {response:?}");
    };
    assert_eq!(agents, [agent.clone()]);

    let response = owner
        .call(|peer| client::members::remove_agent(peer, organization, agent.account))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let mut again = harness.client().await;
    let response = again
        .call(|peer| client::account::authenticate(peer, Some(token)))
        .await;
    assert!(
        matches!(response, Err(_)),
        "a removed agent's token still works: {response:?}"
    );

    harness.stop().await;
}
