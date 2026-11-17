use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_block_agent_is_deleted_with_its_grant() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;

    let ungranted = owner.create_block(BlockParent::Root).await;
    let (account, token) = owner.block_agent(organization, ungranted).await.unwrap();
    let mut agent = harness.second_connection(&token, organization).await;
    owner
        .grant(ungranted, account, Access::View)
        .await
        .expect("lowering the agent's grant failed");
    assert!(
        TestClient::signs_in(&harness, &token).await,
        "a lower grant deleted the agent"
    );
    owner
        .grant(ungranted, account, Access::None)
        .await
        .expect("removing the agent's grant failed");
    agent.ended().await;
    assert!(
        !TestClient::signs_in(&harness, &token).await,
        "an agent outlived its grant"
    );

    let uncontacted = owner.create_block(BlockParent::Root).await;
    let (account, token) = owner.block_agent(organization, uncontacted).await.unwrap();
    let response = owner.remove_contact(account).await;
    assert!(response.is_ok(), "{response:?}");
    assert!(
        !TestClient::signs_in(&harness, &token).await,
        "an agent outlived the grant its removal from contacts took"
    );

    harness.stop().await;
}
