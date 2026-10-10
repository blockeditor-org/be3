use serde_json::json;

use super::*;
use crate::cli::{Statuses, pull_request, row_ids};

#[tokio::test]
async fn a_pull_request_moves_the_rows_it_names() {
    assert_eq!(
        row_ids("claude/be-7k3q2m-fix\nFixes BE-ABCDEF and be-7K3Q2M, not BE-12 or X-1234567"),
        ["BE-7K3Q2M", "BE-ABCDEF"]
    );

    let harness = Harness::start().await;
    let tracker = Tracker::new(harness.owner().await);
    let board = board(&tracker).await;
    let peer = tracker.peer();
    let schema = peer
        .open::<DatabaseContent>(board.database)
        .await
        .unwrap()
        .unwrap()
        .root()
        .schema
        .unwrap();
    let mut content = peer
        .open::<DatabaseSchemaContent>(schema)
        .await
        .unwrap()
        .unwrap();
    let head = peer.local_head(schema);
    content.apply(&DatabaseSchema::add_enum_option(board.status, "In review").1);
    peer.save(schema, &content, head).await.unwrap();
    let made = tracker
        .create(board.database, "Fix the login", &[], Body::Keep)
        .await
        .unwrap();
    let statuses = Statuses {
        field: "Status".into(),
        review: "In review".into(),
        done: "Done".into(),
    };
    let event = |action: &str, merged: bool| {
        json!({
            "action": action,
            "pull_request": {
                "html_url": "https://github.com/example/repo/pull/7",
                "title": "Fix the login",
                "body": format!("Fixes {}.", made.id),
                "head": {"ref": "claude/fix-login"},
                "merged": merged,
                "draft": false,
            },
        })
    };

    pull_request(&tracker, &event("opened", false), &statuses)
        .await
        .unwrap();
    let opened = tracker.show(&made.id).await.unwrap();
    assert_eq!(
        opened.fields,
        [("Status".to_owned(), "In review".to_owned())]
    );
    assert_eq!(
        opened.body.as_deref(),
        Some("Pull request: https://github.com/example/repo/pull/7\n")
    );

    pull_request(&tracker, &event("reopened", false), &statuses)
        .await
        .unwrap();
    pull_request(&tracker, &event("closed", true), &statuses)
        .await
        .unwrap();
    let merged = tracker.show(&made.id).await.unwrap();
    assert_eq!(merged.fields, [("Status".to_owned(), "Done".to_owned())]);
    assert_eq!(merged.body, opened.body, "the link is added once");
    harness.stop().await;
}
