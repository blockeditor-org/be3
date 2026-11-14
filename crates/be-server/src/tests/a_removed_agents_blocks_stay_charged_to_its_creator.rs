use super::*;

#[tokio::test]
async fn a_removed_agents_blocks_stay_charged_to_its_creator() {
    let harness = Harness::start().await;
    let mut creator = harness.client().await;
    let account = creator.register("keeper@example.com").await;
    let organization = creator.organization("team").await;
    let response = creator
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
    let Ok(be_block_protocol::CreatedAgent { agent, token }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    let mut helper = harness.second_connection(&token, organization).await;
    let block = helper.create_block(BlockParent::Root).await;
    drop(helper);
    let removed = creator
        .call(|peer| client::members::remove_member(peer, agent.account))
        .await;
    assert!(removed.is_ok(), "{removed:?}");

    let author = Author::new(account);
    let (head, chunks) = author.compose(b"written after the agent was removed\n", 1_000, None);
    let published = creator
        .publish(&author, block, head, chunks.clone(), None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let size: u64 = author
        .objects_of(head, &chunks)
        .iter()
        .map(|hash| author.store.get(*hash).unwrap().unwrap().len() as u64)
        .sum();
    let usage = creator.storage().await.unwrap();
    assert_eq!(
        usage.held, size,
        "a removed agent's block is still charged to whoever created the agent"
    );

    harness.stop().await;
}
