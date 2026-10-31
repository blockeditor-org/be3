use be_graph::Access;
use be_protocol::{CreatedAgent, client};

use super::*;
use crate::session::{Settings, connect};

#[tokio::test]
async fn an_agent_reads_and_writes_the_rows_shared_with_it() {
    let harness = Harness::start().await;
    let owner = Tracker::new(harness.owner().await, sources());
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
    let CreatedAgent { agent, token } = be_client::ask(client::members::create_agent(
        &owner.peer().connection(),
        organization,
        "Claude".into(),
        public.0,
        be_client::contacts::card(
            owner.peer().account(),
            owner.peer().keys().identity(),
            "Owner",
            "owner@example.com",
        )
        .seal_own(&phrase.secret()),
        owner
            .peer()
            .keys()
            .escrow()
            .map_or_else(Vec::new, |escrow| {
                be_client::contacts::seal_pin(&phrase.secret(), organization, &escrow)
            }),
    ))
    .await
    .expect("creating the agent failed");
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

    let credentials = serde_json::json!({
        "server": harness.url,
        "organization": owner.peer().organization(),
        "token": token,
        "recovery_phrase": phrase.to_string(),
    })
    .to_string();
    let cache = std::env::temp_dir().join(format!("be-cli-cache-{}", Uuid::new_v4()));
    let settings = Settings::from_values(|name| match name {
        "BE3_AGENT_CREDENTIALS" => Some(credentials.clone()),
        "BE3_CACHE" => Some(cache.to_string_lossy().into_owned()),
        _ => None,
    })
    .unwrap();
    let agent = Tracker::new(connect(&settings).await.unwrap(), sources());
    assert_eq!(agent.find_database("Issues").await.unwrap(), board.database);
    assert!(agent.entry(private).await.is_err());

    let made = agent
        .create(
            board.database,
            "Found by the agent",
            &[set("Status", "To do")],
            Body::Replace("Steps to reproduce.".into()),
        )
        .await
        .unwrap();
    let read = owner.show(board.database, &made.id).await.unwrap();
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
    let notes = owner
        .children(agents)
        .await
        .unwrap()
        .into_iter()
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
