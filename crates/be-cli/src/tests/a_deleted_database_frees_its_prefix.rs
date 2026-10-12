use super::*;

#[tokio::test]
async fn a_deleted_database_frees_its_prefix() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await);
    run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    let said = run(&tracker, "Issues delete").await.unwrap();
    assert!(said.starts_with("deleted Issues"), "{said}");
    assert_eq!(run(&tracker, "ls").await.unwrap(), "nothing here");
    let short = said
        .split(['(', ')'])
        .nth(1)
        .expect("the deleted view's id")
        .to_owned();
    assert!(
        run(&tracker, "Issues info").await.is_err(),
        "a deleted block is still found by its name"
    );
    let info = run(&tracker, &format!("{short} info")).await.unwrap();
    assert!(info.contains("in nothing (deleted)"), "{info}");

    run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    let database = tracker.find_database("Issues").await.unwrap();
    let made = tracker
        .create(database, "After the old one", &[], Body::Keep)
        .await
        .unwrap();
    assert!(made.id.starts_with("BE-"), "{}", made.id);
    harness.stop().await;
}
