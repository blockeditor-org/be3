use super::*;

#[tokio::test]
async fn an_import_links_rows_to_each_other_by_their_old_ids() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    run(&tracker, "Issues add-column ID text").await.unwrap();
    run(
        &tracker,
        "Issues add-column Blocked_by relation --links Issues --multiple --inverse Blocking",
    )
    .await
    .unwrap();
    let records = crate::csv::parse(
        "ID,Title,Blocked by\n\
         OLD-1,Fix the login,\n\
         OLD-2,Retry on slow networks,OLD-1\n\
         OLD-3,Show a retry banner,\"OLD-1, OLD-2\"\n",
    )
    .unwrap();
    let database = tracker.find_database("Issues").await.unwrap();
    let imported = tracker
        .import(database, records, "Description")
        .await
        .unwrap();
    assert!(imported.problems.is_empty(), "{:?}", imported.problems);

    let rows = tracker.list(database, &[]).await.unwrap();
    let banner = rows
        .iter()
        .find(|row| row.title == "Show a retry banner")
        .unwrap();
    let mut blockers: Vec<String> = banner
        .field("Blocked by")
        .unwrap()
        .links
        .iter()
        .map(|(_, title)| title.clone())
        .collect();
    blockers.sort();
    assert_eq!(blockers, ["Fix the login", "Retry on slow networks"]);
    let login = rows
        .iter()
        .find(|row| row.title == "Fix the login")
        .unwrap();
    assert_eq!(login.field("Blocking").unwrap().links.len(), 2);
    let blocked = run(&tracker, "Issues rows Blocked_by=OLD-1").await.unwrap();
    assert_eq!(blocked.lines().count(), 2, "{blocked}");
    harness.stop().await;
}
