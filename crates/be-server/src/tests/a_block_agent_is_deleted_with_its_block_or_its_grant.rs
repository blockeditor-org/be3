use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_block_agent_is_deleted_with_its_block_or_its_grant() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;

    let collected = owner.create_block(BlockParent::Root).await;
    let (_, token) = owner.block_agent(organization, collected).await.unwrap();
    let mut agent = harness.second_connection(&token, organization).await;
    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, collected, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(response.is_ok(), "{response:?}");
    let response = owner
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    assert!(
        matches!(&response, Ok(done) if done.blocks.contains(&collected)),
        "{response:?}"
    );
    agent.ended().await;
    assert!(
        !TestClient::signs_in(&harness, &token).await,
        "an agent outlived its collected block"
    );

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
    let response = owner
        .call(|peer| client::contacts::remove_contact(peer, account))
        .await;
    assert!(response.is_ok(), "{response:?}");
    assert!(
        !TestClient::signs_in(&harness, &token).await,
        "an agent outlived the grant its removal from contacts took"
    );

    harness.stop().await;
}
