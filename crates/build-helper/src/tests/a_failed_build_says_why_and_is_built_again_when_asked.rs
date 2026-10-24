use super::*;

#[tokio::test]
async fn a_failed_build_says_why_and_is_built_again_when_asked() {
    let mut running = start("failed", None).await;

    running.product("POST", "android", COMMIT).await;
    running.started().await;
    running.finish(Err("buck build failed (exit status: 3)\nmore".into()));
    assert_eq!(
        running
            .finished("android", COMMIT)
            .await
            .header("build-state"),
        Some("failed")
    );
    assert_eq!(
        running.product("GET", "android", COMMIT).await.json(),
        serde_json::json!({
            "product": "android",
            "commit": COMMIT,
            "state": "failed",
            "error": "buck build failed (exit status: 3)",
        })
    );

    let again = running.product("POST", "android", COMMIT).await.json();
    assert!(
        again["state"] == "queued" || again["state"] == "running",
        "{again}"
    );
    running.started().await;
    assert_eq!(running.built().len(), 2);
    running.finish(Ok(()));
    running.stop().await;
}
