use super::*;

#[tokio::test]
async fn the_position_counts_the_builds_ahead() {
    let mut running = start("position", None).await;

    let first = running.product("POST", "android", COMMIT).await.json();
    assert!(
        first["state"] == "queued" || first["state"] == "running",
        "{first}"
    );
    running.started().await;
    let second = running.product("POST", "android", OTHER).await.json();
    let third = running
        .product("POST", "beui-demo-macos_arm64", COMMIT)
        .await
        .json();
    assert_eq!(second["state"], "queued");
    assert_eq!(second["position"], 1);
    assert_eq!(third["position"], 2);
    assert!(
        running
            .product("GET", "android", COMMIT)
            .await
            .json()
            .get("position")
            .is_none()
    );

    running.finish(Ok(()));
    assert_eq!(running.started().await, format!("android/{OTHER}"));
    let third = running
        .product("GET", "beui-demo-macos_arm64", COMMIT)
        .await
        .json();
    assert_eq!(third["state"], "queued");
    assert_eq!(third["position"], 1);

    running.finish(Ok(()));
    assert_eq!(
        running.started().await,
        format!("beui-demo-macos_arm64/{COMMIT}")
    );
    running.finish(Ok(()));
    running.stop().await;
}
