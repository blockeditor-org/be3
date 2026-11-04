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
    let Ok(be_block_protocol::CreatedAgent { token, .. }) = response else {
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

    harness.stop().await;
}
