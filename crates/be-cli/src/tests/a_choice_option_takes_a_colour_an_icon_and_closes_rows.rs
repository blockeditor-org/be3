use super::*;

#[tokio::test]
async fn a_choice_option_takes_a_colour_an_icon_and_closes_rows() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    board(&tracker).await;
    let status = |schema: &str| {
        schema
            .lines()
            .find(|line| line.starts_with("Status\t"))
            .unwrap()
            .to_owned()
    };

    let schema = run(&tracker, "Issues schema").await.unwrap();
    assert_eq!(status(&schema), "Status\tone of To do (gray), Done (gray)");

    let schema = run(
        &tracker,
        "Issues option Status done closed=yes icon=done color=Blue",
    )
    .await
    .unwrap();
    assert_eq!(
        status(&schema),
        "Status\tone of To do (gray), Done (blue, icon done, closed)"
    );

    let schema = run(
        &tracker,
        "Issues add-option Status Backlog first=yes default=yes icon=backlog",
    )
    .await
    .unwrap();
    assert_eq!(
        status(&schema),
        "Status\tone of Backlog (gray, icon backlog, default), To do (gray), Done (blue, icon done, closed)"
    );

    let schema = run(&tracker, "Issues option Status To_do after=Done name=Todo")
        .await
        .unwrap();
    assert_eq!(
        status(&schema),
        "Status\tone of Backlog (gray, icon backlog, default), Done (blue, icon done, closed), Todo (gray)"
    );

    let schema = run(&tracker, "Issues option Status Backlog default=no")
        .await
        .unwrap();
    assert_eq!(
        status(&schema),
        "Status\tone of Backlog (gray, icon backlog), Done (blue, icon done, closed), Todo (gray)"
    );

    let refused = run(&tracker, "Issues option Status Bakclog icon=backlog")
        .await
        .expect_err("a misspelt option is refused, not added");
    assert!(
        refused.contains("Bakclog is not one of Status"),
        "{refused}"
    );
    let refused = run(&tracker, "Issues add-option Status Done")
        .await
        .expect_err("an option that exists is not added twice");
    assert!(refused.contains("Status already has Done"), "{refused}");

    let refused = run(&tracker, "Issues option Status Done color=mauve")
        .await
        .unwrap_err();
    assert!(
        refused.contains("gray, blue, teal, green, yellow, orange, red, pink, purple"),
        "{refused}"
    );
    let refused = run(&tracker, "Issues option Status Done icon=rocket")
        .await
        .unwrap_err();
    assert!(refused.contains("none, backlog, todo"), "{refused}");
    let refused = run(&tracker, "Issues option Status Done closed=maybe")
        .await
        .unwrap_err();
    assert!(refused.contains("yes or no"), "{refused}");
    let refused = run(&tracker, "Issues option Title Done closed=yes")
        .await
        .unwrap_err();
    assert!(refused.contains("no choice column"), "{refused}");
    let refused = run(&tracker, "Issues option Status Done after=Later")
        .await
        .unwrap_err();
    assert!(refused.contains("Backlog, Done, Todo"), "{refused}");

    harness.stop().await;
}
