use super::*;

#[tokio::test]
async fn a_folder_syncs_with_a_directory() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let parse = |words: &[&str]| {
        cli::parse(
            words.iter().map(|word| (*word).to_owned()).collect(),
            || Ok(String::new()),
        )
        .unwrap()
    };
    assert_eq!(
        parse(&["Tracker", "sync", "dir", "--watch"]).options.watch,
        Some(10)
    );
    assert_eq!(
        parse(&["sync", "dir", "--watch", "3"]).options.watch,
        Some(3)
    );
    assert_eq!(parse(&["sync", "dir"]).target, None);

    let directory = harness.directory.join("checkout");
    std::fs::create_dir_all(directory.join("notes")).unwrap();
    std::fs::write(directory.join("notes/plan.md"), "the plan\n").unwrap();
    let path = directory.to_str().unwrap().to_owned();
    let synced = run(&tracker, &format!("Tracker sync {path}"))
        .await
        .unwrap();
    assert!(synced.contains("2 created"), "{synced}");
    let plan = tracker.resolve("plan.md").await.unwrap();
    assert_eq!(tracker.text(plan.id).await.unwrap(), "the plan\n");

    run(&tracker, &format!("{} write changed", plan.short))
        .await
        .unwrap();
    let again = run(&tracker, &format!("sync {path}")).await.unwrap();
    assert!(again.contains("1 written"), "{again}");
    assert_eq!(
        std::fs::read_to_string(directory.join("notes/plan.md")).unwrap(),
        "changed"
    );
    let refused = run(&tracker, "sync /nowhere/synced/before")
        .await
        .unwrap_err();
    assert!(refused.contains("not kept in step"), "{refused}");
    harness.stop().await;
}
