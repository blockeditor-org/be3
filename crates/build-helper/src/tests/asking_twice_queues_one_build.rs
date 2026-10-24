use super::*;

#[tokio::test]
async fn asking_twice_queues_one_build() {
    let mut running = start("twice", None).await;

    let first = running.product("POST", "native-linux_x86_64", COMMIT).await;
    assert_eq!(first.status, "HTTP/1.1 200 OK");
    assert_eq!(
        running.started().await,
        format!("native-linux_x86_64/{COMMIT}")
    );
    let second = running.product("POST", "native-linux_x86_64", COMMIT).await;
    assert_eq!(second.json()["state"], "running");
    assert_eq!(
        running
            .product("GET", "native-linux_x86_64", COMMIT)
            .await
            .json()["state"],
        "running"
    );

    running.finish(Ok(()));
    assert_eq!(
        running
            .finished("native-linux_x86_64", COMMIT)
            .await
            .header("build-state"),
        Some("succeeded")
    );
    let third = running.product("POST", "native-linux_x86_64", COMMIT).await;
    assert_eq!(third.json()["state"], "succeeded");
    assert_eq!(running.built(), [format!("native-linux_x86_64/{COMMIT}")]);

    running.stop().await;
}
