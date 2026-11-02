use super::*;

#[tokio::test]
async fn a_new_row_takes_each_choice_fields_default() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let board = board(&tracker).await;
    run(&tracker, "Issues option Status To_do default=yes")
        .await
        .unwrap();

    let plain = tracker
        .create(board.database, "Fix the login", &[], Body::Keep)
        .await
        .unwrap();
    assert_eq!(pairs(&plain), [("Status".to_owned(), "To do".to_owned())]);

    let done = tracker
        .create(
            board.database,
            "Write the guide",
            &[set("status", "Done")],
            Body::Keep,
        )
        .await
        .unwrap();
    assert_eq!(pairs(&done), [("Status".to_owned(), "Done".to_owned())]);

    let cleared = tracker
        .create(board.database, "Someday", &[set("Status", "")], Body::Keep)
        .await
        .unwrap();
    assert!(pairs(&cleared).is_empty());

    let records = crate::csv::parse("Title,Status\nImported,\nFinished,Done\n").unwrap();
    tracker
        .import(board.database, records, "Description")
        .await
        .unwrap();
    let rows = tracker.list(board.database, &[]).await.unwrap();
    let status = |title: &str| {
        rows.iter()
            .find(|row| row.title == title)
            .and_then(|row| row.field("Status"))
            .map(|field| field.text.clone())
    };
    assert_eq!(status("Imported").as_deref(), Some("To do"));
    assert_eq!(status("Finished").as_deref(), Some("Done"));

    harness.stop().await;
}
