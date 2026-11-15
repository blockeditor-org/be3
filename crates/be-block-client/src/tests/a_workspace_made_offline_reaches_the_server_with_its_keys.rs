use super::*;

use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn a_workspace_made_offline_reaches_the_server_with_its_keys() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    laptop.children(BlockParent::Root).await.unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let team = workspace(&offline, "Made offline").await;
    let note = offline
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    assert!(matches!(
        offline.save(note, &TextBlock::of("written offline"), None).await,
        Err(ClientError::Disconnected(_))
    ));
    let roots = offline.children(BlockParent::Root).await.unwrap();
    assert!(roots.iter().any(|root| root.id == team));
    assert_eq!(text(&offline, note).await, "written offline");

    harness.reconnect(&offline, owner.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(drained.contains(&Drained::Created { block: team }));
    assert!(drained.contains(&Drained::Created { block: note }));
    assert_eq!(offline.pending(), refs::PendingCounts::default());

    let phone = harness.second(&owner).await;
    let roots = phone.children(BlockParent::Root).await.unwrap();
    assert!(roots.iter().any(|root| root.id == team));
    assert_eq!(
        text(&phone, note).await,
        "written offline",
        "the workspace's keys did not reach the server"
    );
    harness.stop().await;
}
