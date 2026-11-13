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
    assert!(
        top.lines()
            .any(|line| line.contains("may write") && line.contains("Tracker")),
        "{top}"
    );
    let listed: serde_json::Value =
        serde_json::from_str(&run(&tracker, "ls --json").await.unwrap()).unwrap();
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "Tracker" && entry["writable"] == true),
        "{listed}"
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
    let info = run(&tracker, &format!("{id} page info")).await.unwrap();
    assert!(
        info.contains("kind text"),
        "a row's page is a text block target: {info}"
    );
    let refused = run(&tracker, &format!("{id} set --body Replaced."))
        .await
        .expect_err("set changes fields only");
    assert!(refused.contains("page write"), "{refused}");
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
    run(&tracker, &format!("{id} page append _Seen_twice."))
        .await
        .unwrap();
    let text = run(&tracker, &format!("{id} page")).await.unwrap();
    assert!(
        text.contains("Steps to reproduce.") && text.ends_with("Seen twice."),
        "{text}"
    );
    for refused in [format!("{id} page delete"), format!("{id} page move Tracker")] {
        let refused = run(&tracker, &refused)
            .await
            .expect_err("a row's page takes only the text verbs");
        assert!(refused.contains("a row's page takes cat"), "{refused}");
    }
    let refused = run(&tracker, &format!("{page} delete"))
        .await
        .expect_err("a row's page goes with its row");
    assert!(refused.contains("goes with its row"), "{refused}");
    let refused = run(&tracker, &format!("{id} set --new"))
        .await
        .expect_err("--new is option's alone");
    assert!(refused.contains("--new goes with option"), "{refused}");
    assert!(
        run(&tracker, &format!("{id} page"))
            .await
            .unwrap()
            .ends_with("Seen twice."),
        "the refused page stays where it was"
    );
    let bare = run(&tracker, &format!("{view} new Bare_row"))
        .await
        .unwrap();
    let bare = bare.split_whitespace().next().unwrap().to_owned();
    let refused = run(&tracker, &format!("{bare} page info"))
        .await
        .expect_err("a row with no page has nothing to describe");
    assert!(refused.contains("has no page yet"), "{refused}");
    assert_eq!(run(&tracker, &format!("{bare} page")).await.unwrap(), "");
    let refused = run(&tracker, &format!("{bare} set --append More"))
        .await
        .expect_err("set no longer appends");
    assert!(refused.contains("page append"), "{refused}");
    let wrong = run(&tracker, &format!("{page} rows")).await.unwrap_err();
    assert!(wrong.contains("is a text"), "{wrong}");
    assert!(run(&tracker, "Nothing_like_this").await.is_err());
    harness.stop().await;
}
