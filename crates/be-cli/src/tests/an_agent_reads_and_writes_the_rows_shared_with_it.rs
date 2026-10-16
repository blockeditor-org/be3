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
    let organization = owner.peer().organization();
    let response = owner
        .peer()
        .connection()
        .request(|request| ClientMessage::CreateAgent {
            request,
            organization,
            name: "Claude".into(),
            recovery: public.0,
            contact: be_client::contacts::card(
                owner.peer().account(),
                owner.peer().keys().identity(),
                "Owner",
                "owner@example.com",
            )
            .seal_own(&phrase.secret()),
            escrow: owner
                .peer()
                .keys()
                .escrow()
                .map_or_else(Vec::new, |escrow| {
                    be_client::contacts::seal_pin(&phrase.secret(), organization, &escrow)
                }),
        })
        .await
        .unwrap();
    let ServerMessage::AgentCreated { agent, token, .. } = response else {
        panic!("creating the agent failed: {response:?}");
    };
    owner
        .peer()
        .share(board.database, agent.account, Access::Edit, &public)
        .await
        .unwrap();
    let agents = workspace(owner.peer(), "Agents").await;
    owner
        .peer()
        .share(agents, agent.account, Access::Edit, &public)
        .await
        .unwrap();

    let settings = Settings {
        server: harness.url.clone(),
        organization: None,
        token,
        phrase: phrase.to_string(),
        authorization: None,
        cache: std::env::temp_dir().join(format!("be-cli-cache-{}", Uuid::new_v4())),
    };
    let agent = Tracker::new(connect(&settings).await.unwrap());
    let seen = agent.databases().await.unwrap();
    assert_eq!(
        seen.iter()
            .map(|database| database.label())
            .collect::<Vec<_>>(),
        ["Issues"]
    );
    assert!(seen.iter().all(|database| database.id != private));

    let made = agent
        .create(
            board.database,
            "Found by the agent",
            &[set("Status", "To do")],
            Body::Replace("Steps to reproduce.".into()),
        )
        .await
        .unwrap();
    let read = owner.show(&made.id).await.unwrap();
    assert_eq!(read.title, "Found by the agent");
    assert_eq!(read.body.as_deref(), Some("Steps to reproduce."));

    let top = run(&agent, "ls").await.unwrap();
    assert!(
        top.contains("Agents") && top.contains("Issues") && !top.contains("Tracker"),
        "the agent sees the workspace and the database shared with it, and nothing else: {top}"
    );
    assert!(run(&agent, "workspaces").await.unwrap().ends_with("Agents"));
    run(&agent, "new-database Notes --prefix NO").await.unwrap();
    owner.invalidate().await;
    let index = owner.index().await.unwrap();
    let notes = index
        .entries
        .iter()
        .find(|entry| entry.label() == "Notes" && entry.kind() == "database view")
        .expect("the owner sees the database the agent made");
    assert_eq!(notes.parent, BlockParent::Block(agents));

    let wrong = Settings {
        phrase: be_keys::RecoveryPhrase::generate().to_string(),
        ..settings
    };
    assert!(connect(&wrong).await.is_err());
    harness.stop().await;
}
