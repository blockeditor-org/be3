use super::*;

#[tokio::test]
async fn a_published_manifest_answers_succeeded_without_queueing() {
    let running = start("published", None).await;
    running
        .fake
        .published
        .lock()
        .unwrap()
        .insert(format!("android/{COMMIT}"));

    for method in ["GET", "POST"] {
        let response = running.product(method, "android", COMMIT).await;
        assert_eq!(response.status, "HTTP/1.1 200 OK");
        assert_eq!(
            response.json(),
            serde_json::json!({
                "product": "android",
                "commit": COMMIT,
                "state": "succeeded",
                "manifest": format!("https://cdn.example/builds/android/{COMMIT}.json"),
            })
        );
    }
    assert!(running.built().is_empty());

    running.stop().await;
}
