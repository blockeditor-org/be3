use super::*;

#[tokio::test]
async fn blocks_are_listed_and_found_by_short_id_or_name() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let made = run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    assert!(made.starts_with("made Issues"), "{made}");

    let top = run(&tracker, "ls").await.unwrap();
    assert!(
        top.lines()
            .any(|line| line.contains("workspace") && line.contains("Tracker")),
        "{top}"
    );
    assert!(!top.contains("settings"), "{top}");
    let workspaces = run(&tracker, "workspaces").await.unwrap();
    assert!(
        workspaces.contains("may write") && workspaces.ends_with("Tracker"),
        "{workspaces}"
    );
    let root = run(&tracker, "Tracker ls").await.unwrap();
    let line = root.lines().find(|line| line.contains("Issues")).unwrap();
    assert!(line.contains("database view"), "{line}");
    let view = line.split_whitespace().next().unwrap().to_owned();
    assert!(view.len() >= 4);

    let inside = run(&tracker, &format!("{view} ls")).await.unwrap();
    assert!(inside.contains("database"), "{inside}");
    assert!(inside.contains("rows BE-"), "{inside}");

    let row = run(&tracker, &format!("{view} new Fix_the_login"))
        .await
        .unwrap();
    let id = row.split_whitespace().next().unwrap().to_owned();
    assert!(id.starts_with("BE-"), "{row}");
    assert_eq!(
        run(&tracker, &format!("{view} rows")).await.unwrap(),
        format!("{id}\tFix the login")
    );
    run(
        &tracker,
        &format!("{view} row {id} page write Steps_to_reproduce."),
    )
    .await
    .unwrap();
    let pages = run(&tracker, "Issues ls").await.unwrap();
    assert!(pages.contains("1 row pages"), "{pages}");
    let database = tracker.find_database("Issues").await.unwrap();
    let shown = tracker.show(database, &id).await.unwrap();
    let page = tracker.entry(shown.page.unwrap()).await.unwrap().short;
    assert_eq!(
        run(&tracker, &format!("{page} cat")).await.unwrap(),
        "Steps to reproduce."
    );
    run(&tracker, &format!("{page} append More."))
        .await
        .unwrap();
    assert_eq!(
        run(&tracker, &format!("Issues row {id} page"))
            .await
            .unwrap(),
        "Steps to reproduce.\nMore."
    );
    let wrong = run(&tracker, &format!("{page} rows")).await.unwrap_err();
    assert!(wrong.contains("is a text"), "{wrong}");
    assert!(run(&tracker, "Nothing_like_this").await.is_err());
    harness.stop().await;
}
