use super::*;

#[tokio::test]
async fn unknown_products_and_commits_are_not_found() {
    let running = start("unknown", None).await;

    for (product, commit) in [
        ("ios", COMMIT),
        ("native-plan9_x86_64", COMMIT),
        ("android", "main"),
        ("android", &COMMIT.to_uppercase()),
    ] {
        let response = running.product("POST", product, commit).await;
        assert_eq!(response.status, "HTTP/1.1 404 Not Found");
        assert_eq!(response.json()["state"], "unknown");
        assert!(response.json()["error"].is_string());
    }
    let log = running
        .request(
            "GET",
            &format!("/build/products/android/{THIRD}/log?from=0"),
        )
        .await;
    assert_eq!(log.status, "HTTP/1.1 404 Not Found");
    assert!(running.built().is_empty());

    running.stop().await;
}
