use super::*;

#[tokio::test]
async fn a_block_agents_new_connection_replaces_the_open_one() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let (_, token) = owner.block_agent(organization, terminal).await.unwrap();

    let mut first = harness.second_connection(&token, organization).await;
    first.join_session(terminal).await;
    let mut second = harness.second_connection(&token, organization).await;
    first.ended().await;
    let (_, state) = second.join_session(terminal).await;
    assert_eq!(
        state.participants.len(),
        1,
        "the replaced connection is still in the block's session"
    );
    assert_eq!(second.read_block(terminal).await.id, terminal);

    let response = owner
        .call(|peer| {
            client::members::create_agent(
                peer,
                organization,
                "Helper".into(),
                [7; 32],
                [7; 32],
                Vec::new(),
                Vec::new(),
                None,
            )
        })
        .await;
    let Ok(created) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    let mut one = harness
        .second_connection(&created.token, organization)
        .await;
    let mut two = harness
        .second_connection(&created.token, organization)
        .await;
    one.round_trip().await;
    two.round_trip().await;

    harness.stop().await;
}
