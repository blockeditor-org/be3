use super::*;

#[tokio::test]
async fn an_agent_stores_against_its_creators_quota() {
    let harness = Harness::start().await;
    let mut creator = harness.member("creator@example.com").await;
    let personal = creator.personal().await;
    creator.open(personal).await;
    let response = creator
        .call(|peer| {
            client::members::create_agent(
                peer,
                personal,
                "Helper".into(),
                [7; 32],
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    let Ok(be_block_protocol::CreatedAgent { agent: made, token }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    let mut agent = harness.second_connection(&token, personal).await;

    let upload = b"what the agent wrote".to_vec();
    let stored = agent
        .call(|peer| client::objects::put_objects(peer, vec![upload.clone()]))
        .await;
    assert!(stored.is_ok(), "{stored:?}");
    let usage = creator
        .call(|peer| client::account::storage_usage(peer))
        .await
        .unwrap();
    assert_eq!(
        usage.pending,
        upload.len() as u64,
        "an agent's upload is charged to who created it"
    );
    let usage = agent
        .call(|peer| client::account::storage_usage(peer))
        .await
        .unwrap();
    assert_eq!(
        usage.pending,
        upload.len() as u64,
        "an agent sees its payer's usage"
    );

    let folder = creator.create_block(BlockParent::Root).await;
    let granted = creator
        .grant(folder, made.account, be_block_graph::Access::Edit)
        .await;
    assert!(granted.is_ok(), "{granted:?}");
    let inside = agent.create_block(BlockParent::Block(folder)).await;
    let writer = Author::new(made.account);
    let (head, chunks) = writer.compose(
        b"the agent's note in its creator's folder
",
        1_000,
        None,
    );
    let published = agent
        .publish(&writer, inside, head, chunks.clone(), None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let size: u64 = writer
        .objects_of(head, &chunks)
        .iter()
        .map(|hash| writer.store.get(*hash).unwrap().unwrap().len() as u64)
        .sum();
    let usage = creator
        .call(|peer| client::account::storage_usage(peer))
        .await
        .unwrap();
    assert_eq!(
        usage.held, size,
        "an agent's block inside its creator's tree is charged to the creator"
    );

    harness.stop().await;
}
