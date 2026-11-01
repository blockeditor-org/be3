use super::*;

#[tokio::test]
async fn members_manage_only_the_agents_they_created() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let mut other = harness.member("other@example.com").await;
    let personal = owner.personal().await;
    assert_eq!(other.personal().await, personal);
    let company = owner.organization("Team").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let other_account = store.resolve_token(&other.token).await.unwrap().account;
    store
        .add_member(company, other_account, OrganizationRole::Editor)
        .await
        .unwrap();
    for organization in [personal, company] {
        let response = other
            .call(|peer| {
                client::members::create_agent(
                    peer,
                    organization,
                    "Helper".into(),
                    [7; 32],
                    Vec::new(),
                    Vec::new(),
                )
            })
            .await;
        let Ok(be_protocol::CreatedAgent { agent, token }) = response else {
            panic!("a member could not create an agent: {response:?}");
        };
        let listed = owner
            .call(|peer| client::members::list_agents(peer, organization))
            .await;
        assert!(matches!(listed, Ok(agents) if agents.is_empty()));
        let listed = other
            .call(|peer| client::members::list_agents(peer, organization))
            .await;
        assert!(
            matches!(listed, Ok(agents) if agents.len() == 1 && agents[0].account == agent.account)
        );
        let denied = owner
            .call(|peer| client::members::remove_agent(peer, organization, agent.account))
            .await;
        assert!(matches!(denied, Err(_)));
        let removed = other
            .call(|peer| client::members::remove_agent(peer, organization, agent.account))
            .await;
        assert!(matches!(removed, Ok(())), "{removed:?}");
        let mut bot = harness.client().await;
        let refused = bot
            .call(|peer| client::account::authenticate(peer, Some(token)))
            .await;
        assert!(matches!(refused, Err(_)));
    }
    harness.stop().await;
}
