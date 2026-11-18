use super::*;
use crate::store::MAX_AGENTS_PER_CREATOR;

#[tokio::test]
async fn an_account_creates_a_bounded_number_of_agents() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store.register("owner@example.com", "Owner").await.unwrap();
    let organization = store.personal_organization().await.unwrap();
    let mut agents = Vec::new();
    for index in 0..MAX_AGENTS_PER_CREATOR {
        let (agent, _, _) = store
            .create_agent(
                profile.account,
                organization,
                agent_id(profile.account, &unused_keys(index as u8)),
                &format!("Agent {index}"),
                [index as u8; 32],
                [index as u8; 32],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
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
                agent_id(profile.account, &unused_keys(200)),
                "One more",
                [200; 32],
                [200; 32],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                None
            )
            .await
            .is_err()
    );
    let role = store
        .membership(profile.account, organization)
        .await
        .unwrap();
    let identity = Identity {
        account: profile.account,
        organization,
        role,
        bound: None,
    };
    store
        .remove_member(identity, agents[0].account, None, Vec::new())
        .await
        .unwrap();
    store
        .create_agent(
            profile.account,
            organization,
            agent_id(profile.account, &unused_keys(201)),
            "Replacement",
            [201; 32],
            [201; 32],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            None,
        )
        .await
        .unwrap();
}
