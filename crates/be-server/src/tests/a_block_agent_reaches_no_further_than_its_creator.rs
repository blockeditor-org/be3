use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_block_agent_reaches_no_further_than_its_creator() {
    let harness = Harness::start().await;
    let mut alice = harness.member("alice@example.com").await;
    let organization = alice.personal().await;
    let mut bob = harness.member("bob@example.com").await;
    let bob_id = bob.id();
    bob.personal().await;
    let link = alice.link(None).await;
    let _ = bob.accept(link, &[]).await;
    let block = alice.create_block(BlockParent::Root).await;

    for lowered in [Access::None, Access::Edit] {
        alice
            .grant(block, bob_id, Access::Manage)
            .await
            .expect("sharing with Bob failed");
        let (agent, token) = bob.block_agent(organization, block).await.unwrap();
        let mut host = harness.second_connection(&token, organization).await;
        alice
            .grant(block, bob_id, lowered)
            .await
            .expect("lowering Bob's grant failed");
        host.ended().await;
        assert!(
            !TestClient::signs_in(&harness, &token).await,
            "Bob's agent outlived Bob managing the block ({lowered:?})"
        );
        let response = bob
            .call(|peer| {
                peer.write_in::<String>(
                    Some(organization),
                    be_block_protocol::Op::RotateAgentToken(agent),
                )
            })
            .await;
        assert!(
            response.is_err(),
            "Bob rotated the token of an agent of a block he no longer manages: {response:?}"
        );
    }

    harness.stop().await;
}
