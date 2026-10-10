use super::*;

#[tokio::test]
async fn rows_are_added_filtered_changed_and_given_pages() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await);
    let board = board(&tracker).await;

    let made = tracker
        .create(
            board.database,
            "Fix the login",
            &[
                ("Status".into(), "to do".into()),
                ("Project".into(), "Launch".into()),
            ],
            Body::Replace("It fails on a slow network.".into()),
        )
        .await
        .unwrap();
    assert!(made.id.starts_with("BE-"), "{}", made.id);
    assert_eq!(made.title, "Fix the login");
    assert_eq!(
        made.fields,
        [
            ("Status".to_owned(), "To do".to_owned()),
            ("Project".to_owned(), "Launch".to_owned()),
        ]
    );
    assert_eq!(made.body.as_deref(), Some("It fails on a slow network."));
    tracker
        .create(board.database, "Write the guide", &[], Body::Keep)
        .await
        .unwrap();

    let open = cli::run(
        &tracker,
        &invocation(Command::List {
            database: "be".into(),
            filters: vec![Filter::parse("Status=To do").unwrap()],
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        open,
        format!(
            "{}\tFix the login\t[Status: To do; Project: Launch]",
            made.id
        )
    );

    let changed = tracker
        .update(
            &made.id.to_lowercase(),
            &[
                ("Status".into(), "Done".into()),
                ("Project".into(), String::new()),
            ],
            Body::Append("Fixed by retrying.".into()),
        )
        .await
        .unwrap();
    assert_eq!(changed.fields, [("Status".to_owned(), "Done".to_owned())]);
    assert_eq!(
        changed.body.as_deref(),
        Some("It fails on a slow network.\nFixed by retrying.")
    );
    let refused = tracker
        .update(&made.id, &[("Status".into(), "Maybe".into())], Body::Keep)
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
    assert_eq!(summary.parent, BlockParent::Block(board.database));
    assert_eq!(
        tracker.peer().metadata(&summary).unwrap().name.as_deref(),
        Some("Fix the login")
    );

    harness.stop().await;
}
