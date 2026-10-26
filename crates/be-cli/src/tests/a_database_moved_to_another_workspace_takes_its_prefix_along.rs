use be_block::{PREFIXES, PrefixRegistryContent};

use super::*;

async fn registered(tracker: &Tracker<MemoryStore>, owner: Uuid) -> Vec<Uuid> {
    let peer = tracker.peer();
    let summary = peer.summary(owner).await.unwrap();
    let Some(kind) = peer.metadata(&summary).unwrap().get(&PREFIXES) else {
        return Vec::new();
    };
    let content = peer
        .open::<PrefixRegistryContent>(kind.0)
        .await
        .unwrap()
        .unwrap_or_default();
    content.root().databases.keys().copied().collect()
}

#[tokio::test]
async fn a_database_moved_to_another_workspace_takes_its_prefix_along() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let first = tracker.top().await.unwrap()[0].id;
    let second = workspace(tracker.peer(), "Elsewhere").await;
    let (view, database) = tracker
        .create_database("Issues", "BE", BlockParent::Block(first))
        .await
        .unwrap();
    let row = tracker
        .create(database, "Moved row", &[], Body::Keep)
        .await
        .unwrap();
    assert_eq!(registered(&tracker, first).await, vec![database]);

    let moved = run(&tracker, &format!("{view} move {second}"))
        .await
        .unwrap();
    assert!(moved.contains("moved Issues"), "{moved}");
    assert_eq!(
        tracker.peer().summary(view).await.unwrap().parent,
        Some(BlockParent::Block(second))
    );
    assert_eq!(registered(&tracker, first).await, Vec::<Uuid>::new());
    assert_eq!(registered(&tracker, second).await, vec![database]);
    let shown = run(&tracker, &row.id).await.unwrap();
    assert!(shown.contains("Moved row"), "{shown}");
    harness.stop().await;
}
