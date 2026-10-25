use super::*;

#[tokio::test]
async fn a_row_blocked_by_another_shows_as_blocking_there() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    run(&tracker, "new-database Issues --prefix be")
        .await
        .unwrap();
    let schema = run(
        &tracker,
        "Issues add-column Blocked_by relation --links Issues --multiple --inverse Blocking",
    )
    .await
    .unwrap();
    assert!(schema.contains("Blocked by\trows of Issues"), "{schema}");
    assert!(schema.contains("Blocking\trows of Issues"), "{schema}");

    let database = tracker.find_database("Issues").await.unwrap();
    let login = tracker
        .create(database, "Fix the login", &[], Body::Keep)
        .await
        .unwrap();
    let retry = tracker
        .create(database, "Retry on slow networks", &[], Body::Keep)
        .await
        .unwrap();
    let banner = tracker
        .create(
            database,
            "Show a retry banner",
            &[Assignment::parse(&format!("Blocked by={},{}", login.id, retry.id)).unwrap()],
            Body::Keep,
        )
        .await
        .unwrap();
    let blockers: Vec<String> = banner
        .field("Blocked by")
        .unwrap()
        .links
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(blockers.len(), 2);
    assert!(blockers.contains(&login.id) && blockers.contains(&retry.id));

    let shown = tracker.show(database, &login.id).await.unwrap();
    let blocking = shown.field("Blocking").unwrap();
    assert_eq!(
        blocking.links,
        [(banner.id.clone(), "Show a retry banner".to_owned())]
    );

    let blocked = run(&tracker, &format!("Issues rows Blocked_by={}", login.id))
        .await
        .unwrap();
    assert!(blocked.starts_with(&banner.id), "{blocked}");

    tracker
        .update(
            database,
            &login.id,
            &[Assignment::parse(&format!("Blocking-={}", banner.id)).unwrap()],
            Body::Keep,
        )
        .await
        .unwrap();
    let banner = tracker.show(database, &banner.id).await.unwrap();
    assert_eq!(banner.field("Blocked by").unwrap().links.len(), 1);
    assert_eq!(banner.field("Blocked by").unwrap().links[0].0, retry.id);
    tracker
        .update(
            database,
            &retry.id,
            &[Assignment::parse(&format!("Blocking+={}", login.id)).unwrap()],
            Body::Keep,
        )
        .await
        .unwrap();
    let login = tracker.show(database, &login.id).await.unwrap();
    assert_eq!(login.field("Blocked by").unwrap().links[0].0, retry.id);

    run(&tracker, &format!("Issues row {} delete", retry.id))
        .await
        .unwrap();
    assert!(
        tracker
            .show(database, &login.id)
            .await
            .unwrap()
            .field("Blocked by")
            .is_none()
    );
    assert!(
        tracker
            .show(database, &banner.id)
            .await
            .unwrap()
            .field("Blocked by")
            .is_none()
    );
    harness.stop().await;
}
