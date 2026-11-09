use super::*;

#[tokio::test]
async fn a_block_agent_is_refused_while_its_block_is_in_the_trash() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let (_, token) = owner.block_agent(organization, terminal).await.unwrap();
    let mut agent = harness.second_connection(&token, organization).await;

    let response = owner
        .call(|peer| client::blocks::delete_block(peer, terminal))
        .await;
    assert!(response.is_ok(), "{response:?}");
    let response = agent
        .call(|peer| client::blocks::read_block(peer, terminal))
        .await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "an agent read its block in the trash: {response:?}"
    );
    let response = agent
        .call(|peer| client::organizations::open_organization(peer, organization))
        .await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "an agent opened its organization while its block is in the trash: {response:?}"
    );
    // Signing in again replaces the connection above, which closes.
    assert!(
        TestClient::signs_in(&harness, &token).await,
        "the trash deleted the block's agent"
    );
    agent.ended().await;

    owner
        .move_to(terminal, BlockParent::Root)
        .await
        .expect("restoring the block failed");
    let mut agent = harness.second_connection(&token, organization).await;
    let response = agent
        .call(|peer| client::blocks::read_block(peer, terminal))
        .await;
    assert!(
        response.is_ok(),
        "a restored block's agent is still refused: {response:?}"
    );

    harness.stop().await;
}
