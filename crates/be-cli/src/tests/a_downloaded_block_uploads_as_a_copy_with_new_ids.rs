use super::*;

#[tokio::test]
async fn a_downloaded_block_uploads_as_a_copy_with_new_ids() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await);
    run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    let row = run(&tracker, "Issues new Fix_the_login").await.unwrap();
    let id = row.split_whitespace().next().unwrap().to_owned();
    run(
        &tracker,
        &format!("Issues row {id} page write Steps_to_reproduce."),
    )
    .await
    .unwrap();

    let file = harness.directory.join("issues.block");
    std::fs::create_dir_all(&harness.directory).unwrap();
    let file = file.to_str().unwrap().to_owned();
    let wrote = run(&tracker, &format!("Issues download {file}"))
        .await
        .unwrap();
    assert!(wrote.contains("blocks inside it"), "{wrote}");

    let original = tracker.find_database("Issues").await.unwrap();
    let uploaded = run(&tracker, &format!("upload {file}")).await.unwrap();
    assert!(uploaded.contains("uploaded Issues"), "{uploaded}");

    let databases = tracker.databases().await.unwrap();
    assert_eq!(databases.len(), 2);
    let copy = databases.iter().find(|entry| entry.id != original).unwrap();
    assert_eq!(copy.prefix.as_deref(), Some("BE"));
    let index = tracker.index().await.unwrap();
    let view = index.get(copy.parent.block().unwrap()).unwrap().clone();
    assert_eq!(tracker.database_of(&view).await.unwrap(), copy.id);
    let rows = tracker.list(copy.id, &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Fix the login");
    let copied = tracker.show(copy.id, &rows[0].id).await.unwrap();
    assert_eq!(copied.body.as_deref(), Some("Steps to reproduce."));
    let shown = tracker.show(original, &id).await.unwrap();
    assert_ne!(copied.page, shown.page);
    run(
        &tracker,
        &format!("{} row {} page write Changed.", copy.short, rows[0].id),
    )
    .await
    .unwrap();
    let original = index.get(original).unwrap().short.clone();
    assert_eq!(
        run(&tracker, &format!("{original} row {id} page"))
            .await
            .unwrap(),
        "Steps to reproduce."
    );

    let elsewhere = Tracker::new(
        Peer::connect(
            PeerConfig::new(
                be_client::socket_url(&harness.url),
                be_keys::RecoveryPhrase::generate().secret(),
                Credentials::Token(
                    be_server::ServerStore::open(&harness.directory)
                        .unwrap()
                        .register("other@example.com", "Other", "correct horse battery")
                        .await
                        .unwrap()
                        .1,
                ),
            ),
            MemoryStore::new(),
        )
        .await
        .unwrap(),
    );
    let refused = run(&elsewhere, &format!("upload {file}"))
        .await
        .unwrap_err();
    assert!(refused.contains("workspace"), "{refused}");
    workspace(elsewhere.peer(), "Theirs").await;
    elsewhere.invalidate().await;
    run(&elsewhere, &format!("upload {file}")).await.unwrap();
    let shown = run(&elsewhere, "Issues rows").await.unwrap();
    assert!(
        shown.starts_with("BE-") && shown.contains("Fix the login"),
        "{shown}"
    );
    let moved = shown.split_whitespace().next().unwrap().to_owned();
    assert_eq!(
        run(&elsewhere, &format!("Issues row {moved} page"))
            .await
            .unwrap(),
        "Steps to reproduce."
    );

    std::fs::write(&file, b"not a block").unwrap();
    assert!(run(&tracker, &format!("upload {file}")).await.is_err());
    harness.stop().await;
}
