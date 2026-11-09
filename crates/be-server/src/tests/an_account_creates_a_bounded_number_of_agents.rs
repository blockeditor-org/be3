use super::*;
use crate::store::MAX_AGENTS_PER_CREATOR;

#[tokio::test]
async fn an_account_creates_a_bounded_number_of_agents() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store
        .register("owner@example.com", "Owner", "a good password")
        .await
        .unwrap();
    let organization = store.personal_organization().await.unwrap();
    let mut agents = Vec::new();
    for index in 0..MAX_AGENTS_PER_CREATOR {
        let (agent, _, _) = store
            .create_agent(
                profile.account,
                organization,
                &format!("Agent {index}"),
                [7; 32],
                Vec::new(),
                Vec::new(),
                None,
            )
            .await
            .unwrap();
        agents.push(agent);
    }
    assert!(
        store
            .create_agent(
                profile.account,
                organization,
                "One more",
                [7; 32],
                Vec::new(),
                Vec::new(),
                None
            )
            .await
            .is_err()
    );
    store
        .remove_agent(profile.account, organization, agents[0].account)
        .await
        .unwrap();
    store
        .create_agent(
            profile.account,
            organization,
            "Replacement",
            [7; 32],
            Vec::new(),
            Vec::new(),
            None,
        )
        .await
        .unwrap();
}
