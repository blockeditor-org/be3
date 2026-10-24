use super::*;

#[tokio::test]
async fn commits_not_on_a_branch_are_refused() {
    let running = start_with("refused", None, true, &[OTHER]).await;

    let refused = running.product("POST", "android", OTHER).await;
    assert_eq!(refused.status, "HTTP/1.1 422 Unprocessable Entity");
    assert_eq!(refused.json()["state"], "refused");
    assert!(refused.json()["error"].as_str().unwrap().contains(OTHER));
    let never = running.product("GET", "android", OTHER).await;
    assert_eq!(never.status, "HTTP/1.1 404 Not Found");
    assert_eq!(never.json(), serde_json::json!({"state": "unknown"}));
    assert!(running.built().is_empty());

    running.stop().await;
}
