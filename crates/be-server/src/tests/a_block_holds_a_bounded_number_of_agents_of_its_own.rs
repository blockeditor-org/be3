use super::*;
use crate::store::{MAX_AGENTS_PER_BLOCK, MAX_AGENTS_PER_CREATOR};

#[tokio::test]
async fn a_block_holds_a_bounded_number_of_agents_of_its_own() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    for index in 0..MAX_AGENTS_PER_CREATOR {
        let response = owner
            .call(|peer| {
                client::members::create_agent(
                    peer,
                    organization,
                    format!("Agent {index}"),
                    [7; 32],
                    Vec::new(),
                    Vec::new(),
                    None,
                )
            })
            .await;
        assert!(response.is_ok(), "{response:?}");
    }

    let mut bound = Vec::new();
    for _ in 0..MAX_AGENTS_PER_BLOCK {
        let response = owner.block_agent(organization, terminal).await;
        let Ok((account, _)) = response else {
            panic!("the creator's own agents kept a block's agent from being made: {response:?}");
        };
        bound.push(account);
    }
    let response = owner.block_agent(organization, terminal).await;
    assert!(
        refused_as(&response, ErrorCode::InvalidRequest),
        "a block took one agent too many: {response:?}"
    );
    let other = owner.create_block(BlockParent::Root).await;
    assert!(owner.block_agent(organization, other).await.is_ok());

    let response = owner
        .call(|peer| client::members::remove_agent(peer, organization, bound[0]))
        .await;
    assert!(response.is_ok(), "{response:?}");
    assert!(owner.block_agent(organization, terminal).await.is_ok());

    let mut viewer = harness.member("viewer@example.com").await;
    let response = viewer.block_agent(organization, terminal).await;
    assert!(
        response.is_err(),
        "an account that does not manage the block made it an agent: {response:?}"
    );

    harness.stop().await;
}
