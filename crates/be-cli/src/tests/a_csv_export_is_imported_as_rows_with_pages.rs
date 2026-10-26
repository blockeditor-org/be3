use super::*;

#[tokio::test]
async fn a_csv_export_is_imported_as_rows_with_pages() {
    let records = crate::csv::parse(
        "\u{feff}ID,Title,Description,Status,Project,Priority\r\n\
         BE3-1,\"Fix the login\",\"It fails\non a \"\"slow\"\" network.\",Todo,Launch,Urgent\r\n\
         BE3-2,Write the guide,,Done,Docs,Low\r\n",
    )
    .unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records[1][2], "It fails\non a \"slow\" network.");

    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let board = board(&tracker).await;
    let imported = tracker
        .import(board.database, records, "Description")
        .await
        .unwrap();

    assert_eq!(imported.rows, 2);
    assert_eq!(imported.pages, 1);
    assert_eq!(imported.options, ["Status: Todo"]);
    assert_eq!(imported.ignored, ["ID", "Priority"]);
    assert!(imported.problems.is_empty(), "{:?}", imported.problems);
    let rows = tracker.list(board.database, &[]).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].title, "Fix the login");
    assert_eq!(imported.linked_rows, ["Project: Docs"]);
    assert_eq!(rows[0].field("Status").unwrap().text, "Todo");
    assert_eq!(rows[0].field("Project").unwrap().links[0].1, "Launch");
    assert_eq!(
        tracker
            .show(board.database, &rows[0].id)
            .await
            .unwrap()
            .body
            .as_deref(),
        Some("It fails\non a \"slow\" network.")
    );
    assert_eq!(rows[1].field("Status").unwrap().text, "Done");
    assert_eq!(rows[1].field("Project").unwrap().links[0].1, "Docs");
    assert_eq!(rows[1].page, None);
    harness.stop().await;
}
