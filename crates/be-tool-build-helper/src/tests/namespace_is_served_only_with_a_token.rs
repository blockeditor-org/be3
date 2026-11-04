use super::*;

#[tokio::test]
async fn namespace_is_served_only_with_a_token() {
    let running = start_with("tokenless", None, false, &[]).await;

    assert_eq!(running.get("GET").await.0, "HTTP/1.1 404 Not Found");
    assert_eq!(running.runs(), 0);

    running.stop().await;
}
