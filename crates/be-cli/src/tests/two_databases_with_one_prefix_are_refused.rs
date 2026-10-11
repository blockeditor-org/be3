use super::*;

#[tokio::test]
async fn two_databases_with_one_prefix_are_refused() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await);
    let peer = tracker.peer();
    let (first, _) = database(
        peer,
        "First",
        "BE",
        &[("Title", DatabaseFieldType::String, &[])],
    )
    .await;
    database(
        peer,
        "Second",
        "BE",
        &[("Title", DatabaseFieldType::String, &[])],
    )
    .await;
    tracker.invalidate().await;
    let made = tracker.create(first, "One", &[], Body::Keep).await.unwrap();

    let refused = tracker.show(&made.id).await.unwrap_err();
    assert!(
        refused.contains("2 databases use the row id prefix"),
        "{refused}"
    );
    let again = run(&tracker, "new-database Third --prefix be")
        .await
        .unwrap_err();
    assert!(again.contains("prefix"), "{again}");
    harness.stop().await;
}
