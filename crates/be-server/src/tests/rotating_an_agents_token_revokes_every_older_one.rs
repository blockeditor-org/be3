use super::*;

#[tokio::test]
async fn rotating_an_agents_token_revokes_every_older_one() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let (account, token) = owner.block_agent(organization, terminal).await.unwrap();
    let mut agent = harness.second_connection(&token, organization).await;

    let mut stranger = harness.member("stranger@example.com").await;
    let response = stranger
        .call(|peer| client::members::rotate_agent_token(peer, organization, account))
        .await;
    assert!(
        response.is_err(),
        "someone else rotated the agent's token: {response:?}"
    );

    let response = owner
        .call(|peer| client::members::rotate_agent_token(peer, organization, account))
        .await;
    let Ok(rotated) = response else {
        panic!("rotating the token failed: {response:?}");
    };
    assert_ne!(rotated, token);
    agent.ended().await;
    assert!(
        !TestClient::signs_in(&harness, &token).await,
        "the older token still signs in"
    );
    let mut agent = harness.second_connection(&rotated, organization).await;
    assert_eq!(agent.read_block(terminal).await.id, terminal);

    harness.stop().await;
}
