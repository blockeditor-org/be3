use super::*;

#[tokio::test]
async fn a_block_agent_never_becomes_an_administrator() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let elsewhere = owner.create_block(BlockParent::Root).await;
    let (account, token) = owner.block_agent(organization, terminal).await.unwrap();

    let response = owner
        .call(|peer| client::members::set_role(peer, account, OrganizationRole::Administrator))
        .await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "an administrator promoted a block's agent: {response:?}"
    );
    let mut agent = harness.second_connection(&token, organization).await;
    let response = agent.try_read(elsewhere).await;
    assert!(
        refused_as(&response, ErrorCode::BlockNotFound),
        "{response:?}"
    );

    let response = owner
        .call(|peer| client::members::remove_member(peer, account))
        .await;
    assert!(response.is_ok(), "{response:?}");
    agent.ended().await;
    assert!(
        !TestClient::signs_in(&harness, &token).await,
        "a block's agent outlived its removal from the organization"
    );

    harness.stop().await;
}
