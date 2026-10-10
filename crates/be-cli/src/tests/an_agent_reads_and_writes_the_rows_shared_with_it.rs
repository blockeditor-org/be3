use be_graph::Access;
use be_protocol::{ClientMessage, ServerMessage};

use super::*;
use crate::session::{Settings, connect};

#[tokio::test]
async fn an_agent_reads_and_writes_the_rows_shared_with_it() {
    let harness = Harness::start().await;
    let owner = Tracker::new(harness.owner().await);
    let board = board(&owner).await;
    let (private, _) = database(
        owner.peer(),
        "Private",
        "PV",
        &[("Name", DatabaseFieldType::String, &[])],
    )
    .await;

    let phrase = be_keys::RecoveryPhrase::generate();
    let public = phrase.secret().public();
    let workspace = owner.peer().workspace();
    let response = owner
        .peer()
        .connection()
        .request(|request| ClientMessage::CreateAgent {
            request,
            workspace,
            name: "Claude".into(),
            recovery: public.0,
            sealed: public.seal(&KEY),
        })
        .await
        .unwrap();
    let ServerMessage::AgentCreated { agent, token, .. } = response else {
        panic!("creating the agent failed: {response:?}");
    };
    owner
        .peer()
        .grant(board.database, agent.account, Access::Edit)
        .await
        .unwrap();

    let settings = Settings {
        server: harness.url.clone(),
        workspace: None,
        token: Some(token),
        phrase: phrase.to_string(),
    };
    let agent = Tracker::new(connect(&settings).await.unwrap());
    let seen = agent.databases().await.unwrap();
    assert_eq!(
        seen.iter()
            .map(|database| database.name.as_str())
            .collect::<Vec<_>>(),
        ["Issues"]
    );
    assert!(seen.iter().all(|database| database.id != private));

    let made = agent
        .create(
            board.database,
            "Found by the agent",
            &[("Status".into(), "To do".into())],
            Body::Replace("Steps to reproduce.".into()),
        )
        .await
        .unwrap();
    let read = owner.show(&made.id).await.unwrap();
    assert_eq!(read.title, "Found by the agent");
    assert_eq!(read.body.as_deref(), Some("Steps to reproduce."));

    let wrong = Settings {
        phrase: be_keys::RecoveryPhrase::generate().to_string(),
        ..settings
    };
    assert!(connect(&wrong).await.is_err());
    harness.stop().await;
}
