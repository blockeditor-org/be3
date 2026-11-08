use super::*;

#[tokio::test]
async fn a_get_reads_a_body_its_length_delimits() {
    let (url, served) =
        serve_once("HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nhello world").await;
    let body = get(&format!("{url}/bucket/object?X-Amz-Signature=abc"), 1024)
        .await
        .unwrap();
    assert_eq!(body, b"hello world");
    let sent = served.await.unwrap();
    assert!(
        sent.head
            .starts_with("GET /bucket/object?X-Amz-Signature=abc HTTP/1.1\r\n"),
        "the request asks for the URL's path and query: {}",
        sent.head
    );
    let host = url.trim_start_matches("http://");
    assert!(sent.head.contains(&format!("\r\nHost: {host}\r\n")));
    assert!(sent.body.is_empty());
}
