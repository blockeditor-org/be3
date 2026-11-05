use super::*;

#[tokio::test]
async fn a_put_sends_its_headers_and_body() {
    let (url, served) = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await;
    let headers = [
        (
            "x-amz-checksum-sha256",
            "LCa0a2j/xo/5m0U8HTBBNBNCLXBkg7+g+YpeiGJm564=",
        ),
        ("Content-Type", "application/octet-stream"),
        // The request's own length is the body's, whatever the caller says.
        ("Content-Length", "99"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()));
    put(&format!("{url}/bucket/object"), &headers, b"bytes".to_vec())
        .await
        .unwrap();
    let sent = served.await.unwrap();
    assert!(
        sent.head.starts_with("PUT /bucket/object HTTP/1.1\r\n"),
        "{}",
        sent.head
    );
    assert!(
        sent.head.contains(
            "\r\nx-amz-checksum-sha256: LCa0a2j/xo/5m0U8HTBBNBNCLXBkg7+g+YpeiGJm564=\r\n"
        )
    );
    assert!(
        sent.head
            .contains("\r\nContent-Type: application/octet-stream\r\n")
    );
    assert!(sent.head.contains("\r\nContent-Length: 5\r\n"));
    assert!(!sent.head.contains("99"), "{}", sent.head);
    assert_eq!(sent.body, b"bytes");

    // A header that would split the request is never sent.
    let split = [("x-amz-meta".to_owned(), "a\r\nInjected: yes".to_owned())];
    assert!(matches!(
        put(&url, &split, Vec::new()).await,
        Err(Error::Invalid(_))
    ));
}
