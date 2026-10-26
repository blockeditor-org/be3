use super::*;

#[tokio::test]
async fn rows_are_added_filtered_changed_and_given_pages() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let board = board(&tracker).await;

    let made = tracker
        .create(
            board.database,
            "Fix the login",
            &[set("Status", "to do"), set("Project", "Launch")],
            Body::Replace("It fails on a slow network.".into()),
        )
        .await
        .unwrap();
    assert!(made.id.starts_with("BE-"), "{}", made.id);
    assert_eq!(made.title, "Fix the login");
    let launch = made.field("Project").unwrap().links[0].0.clone();
    assert!(launch.starts_with("PR-"), "{launch}");
    assert_eq!(
        pairs(&made),
        [
            ("Status".to_owned(), "To do".to_owned()),
            ("Project".to_owned(), format!("{launch} Launch")),
        ]
    );
    assert_eq!(made.body.as_deref(), Some("It fails on a slow network."));
    tracker
        .create(board.database, "Write the guide", &[], Body::Keep)
        .await
        .unwrap();

    let open = run(&tracker, "Issues rows Status=To_do Project=Launch")
        .await
        .unwrap();
    assert_eq!(
        open,
        format!(
            "{}\tFix the login\t[Status: To do; Project: {launch} Launch]",
            made.id
        )
    );
    assert_eq!(
        run(&tracker, &format!("Issues row {} show", made.id))
            .await
            .unwrap(),
        {
            let shown = tracker.show(board.database, &made.id).await.unwrap();
            cli::row_text(&shown)
        }
    );

    let changed = tracker
        .update(
            board.database,
            &made.id.to_lowercase(),
            &[set("Status", "Done"), set("Project", "")],
            Body::Append("Fixed by retrying.".into()),
        )
        .await
        .unwrap();
    assert_eq!(pairs(&changed), [("Status".to_owned(), "Done".to_owned())]);
    assert_eq!(
        changed.body.as_deref(),
        Some("It fails on a slow network.\nFixed by retrying.")
    );
    assert!(
        run(&tracker, "Issues rows Statsu=Done")
            .await
            .unwrap_err()
            .contains("no column")
    );
    assert!(
        tracker
            .create(board.database, " ", &[], Body::Keep)
            .await
            .is_err()
    );
    let refused = tracker
        .update(
            board.database,
            &made.id,
            &[set("Status", "Maybe")],
            Body::Keep,
        )
        .await
        .unwrap_err();
    assert!(refused.contains("To do, Done"), "{refused}");

    let database = tracker
        .peer()
        .open::<DatabaseContent>(board.database)
        .await
        .unwrap()
        .unwrap()
        .root();
    let row = database.find_short_id(&made.id).unwrap();
    let entry = database.rows.get(row).unwrap();
    assert!(entry.value(board.project).is_none());
    assert!(entry.value(board.status).is_some());
    let page = entry.page().unwrap();
    let summary = tracker.peer().summary(page).await.unwrap();
    assert_eq!(summary.parent, Some(BlockParent::Block(board.database)));
    assert_eq!(
        tracker
            .peer()
            .metadata_of_summary(&summary)
            .await
            .unwrap()
            .name()
            .as_deref(),
        Some("Fix the login")
    );
    assert_eq!(
        run(&tracker, &format!("Issues row {} page", made.id))
            .await
            .unwrap(),
        "It fails on a slow network.\nFixed by retrying."
    );

    let deleted = run(&tracker, &format!("Issues row {} delete", made.id))
        .await
        .unwrap();
    assert!(deleted.starts_with("deleted"), "{deleted}");
    assert!(tracker.show(board.database, &made.id).await.is_err());
    assert_eq!(
        tracker.peer().summary(page).await.unwrap().parent,
        Some(BlockParent::Detached)
    );

    harness.stop().await;
}
