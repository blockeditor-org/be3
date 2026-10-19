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
            .send(|request| ClientMessage::CreateAgent {
                request,
                organization,
                name: "Helper".into(),
                recovery: [7; 32],
                contact: Vec::new(),
                escrow: Vec::new(),
            })
            .await;
        let ServerMessage::AgentCreated { agent, token, .. } = response else {
            panic!("a member could not create an agent: {response:?}");
        };
        let listed = owner
            .send(|request| ClientMessage::ListAgents {
                request,
                organization,
            })
            .await;
        assert!(matches!(listed, ServerMessage::Agents { agents, .. } if agents.is_empty()));
        let listed = other
            .send(|request| ClientMessage::ListAgents {
                request,
                organization,
            })
            .await;
        assert!(
            matches!(listed, ServerMessage::Agents { agents, .. } if agents.len() == 1 && agents[0].account == agent.account)
        );
        let denied = owner
            .send(|request| ClientMessage::RemoveAgent {
                request,
                organization,
                account: agent.account,
            })
            .await;
        assert!(matches!(denied, ServerMessage::Failed { .. }));
        let removed = other
            .send(|request| ClientMessage::RemoveAgent {
                request,
                organization,
                account: agent.account,
            })
            .await;
        assert!(matches!(removed, ServerMessage::Ok { .. }), "{removed:?}");
        let mut bot = harness.client().await;
        let refused = bot
            .send(|request| ClientMessage::Authenticate { request, token })
            .await;
        assert!(matches!(refused, ServerMessage::Failed { .. }));
    }
    harness.stop().await;
}
