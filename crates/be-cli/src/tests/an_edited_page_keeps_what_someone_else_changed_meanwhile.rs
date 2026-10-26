use super::*;

#[tokio::test]
async fn an_edited_page_keeps_what_someone_else_changed_meanwhile() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let board = board(&tracker).await;
    let made = tracker
        .create(
            board.database,
            "Plan the release",
            &[],
            Body::Replace("one\ntwo\nthree\n".into()),
        )
        .await
        .unwrap();
    let base = made.body.clone().unwrap();
    let page = made.page.unwrap();

    tracker
        .update(
            board.database,
            &made.id,
            &[],
            Body::Replace("one\ntwo\nthree\nfour\n".into()),
        )
        .await
        .unwrap();
    let clean = tracker
        .rewrite_text(page, &base, "ONE\ntwo\nthree\n")
        .await
        .unwrap();

    assert!(clean);
    assert_eq!(
        tracker
            .show(board.database, &made.id)
            .await
            .unwrap()
            .body
            .as_deref(),
        Some("ONE\ntwo\nthree\nfour\n")
    );
    harness.stop().await;
}
