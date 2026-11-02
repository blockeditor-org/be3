use serde_json::json;

use super::*;
use crate::cli::{Statuses, pull_request};

#[tokio::test]
async fn a_merged_pull_request_closes_rows_without_a_done_status() {
    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await, sources());
    let board = board(&tracker).await;
    run(&tracker, "Issues add-option Status Shipped closed=yes")
        .await
        .unwrap();
    run(&tracker, "Issues option Status Shipped first=yes")
        .await
        .unwrap();
    run(&tracker, "Issues add-option Status Canceled closed=yes")
        .await
        .unwrap();
    let made = tracker
        .create(
            board.database,
            "Fix the login",
            &[set("Status", "To do")],
            Body::Keep,
        )
        .await
        .unwrap();
    let event = json!({
        "action": "closed",
        "pull_request": {
            "html_url": "https://github.com/example/repo/pull/7",
            "title": format!("Fix {}", made.id),
            "body": "",
            "head": {"ref": "claude/fix-login"},
            "merged": true,
            "draft": false,
        },
    });
    let statuses = |done: Option<&str>| Statuses {
        field: "Status".into(),
        review: "In review".into(),
        done: done.map(str::to_owned),
    };

    let said = pull_request(&tracker, board.database, &event, &statuses(None))
        .await
        .unwrap();
    assert_eq!(said, [format!("{}: Status is now Shipped", made.id)]);
    let shipped = tracker.show(board.database, &made.id).await.unwrap();
    assert_eq!(
        pairs(&shipped),
        [("Status".to_owned(), "Shipped".to_owned())]
    );

    pull_request(
        &tracker,
        board.database,
        &event,
        &statuses(Some("Canceled")),
    )
    .await
    .unwrap();
    let canceled = tracker.show(board.database, &made.id).await.unwrap();
    assert_eq!(
        pairs(&canceled),
        [("Status".to_owned(), "Canceled".to_owned())]
    );

    harness.stop().await;
}
