use super::*;

#[tokio::test]
async fn a_row_id_resolves_through_the_workspaces_prefixes() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let workspace = tracker.top().await.unwrap()[0].id;
    let mut made = Vec::new();
    for (name, prefix) in [("Issues", "BE"), ("Milestones", "ms")] {
        let (_, database) = tracker
            .create_database(name, prefix, BlockParent::Block(workspace))
            .await
            .unwrap();
        let row = tracker
            .create(database, &format!("{name} row"), &[], Body::Keep)
            .await
            .unwrap();
        made.push((name, row.id));
    }
    assert!(made[1].1.starts_with("MS-"), "{}", made[1].1);

    for (name, id) in &made {
        let shown = run(&tracker, id).await.unwrap();
        assert!(shown.contains(&format!("{name} row")), "{shown}");
        let named = run(&tracker, &format!("{name} row {id}")).await.unwrap();
        assert_eq!(named, shown);
    }
    let changed = run(&tracker, &format!("{} set Title=Renamed", made[0].1))
        .await
        .unwrap();
    assert!(changed.contains("Renamed"), "{changed}");
    let unknown = run(&tracker, "ZZ-ABCDEF").await.unwrap_err();
    assert!(unknown.contains("no database"), "{unknown}");

    let (_, second) = tracker
        .create_database("Bugs", "BE", BlockParent::Block(workspace))
        .await
        .unwrap();
    tracker.invalidate().await;
    let row = tracker
        .create(second, "Bugs row", &[], Body::Keep)
        .await
        .unwrap();
    let ambiguous = run(&tracker, &row.id).await.unwrap_err();
    assert!(
        ambiguous.contains("more than one database") && ambiguous.contains("Issues"),
        "{ambiguous}"
    );
    let named = run(&tracker, &format!("Bugs row {}", row.id))
        .await
        .unwrap();
    assert!(named.contains("Bugs row"), "{named}");
    harness.stop().await;
}
