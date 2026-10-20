use super::*;

#[tokio::test]
async fn two_databases_may_share_a_prefix_and_each_resolves_its_own_rows() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await);
    let peer = tracker.peer();
    let mut made = Vec::new();
    for name in ["First", "Second"] {
        let (database, _) = database(
            peer,
            name,
            "BE",
            &[("Title", DatabaseFieldType::String, &[])],
        )
        .await;
        tracker.invalidate().await;
        let row = tracker
            .create(database, &format!("{name} row"), &[], Body::Keep)
            .await
            .unwrap();
        assert!(row.id.starts_with("BE-"), "{}", row.id);
        made.push((name, row.id));
    }

    for (name, id) in &made {
        let shown = run(&tracker, &format!("{name} row {id}")).await.unwrap();
        assert!(shown.contains(&format!("{name} row")), "{shown}");
    }
    let bare = run(&tracker, &made[0].1).await.unwrap_err();
    assert!(bare.contains("within its database"), "{bare}");
    harness.stop().await;
}
