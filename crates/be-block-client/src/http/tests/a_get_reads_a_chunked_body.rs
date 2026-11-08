use super::*;

#[tokio::test]
async fn a_get_reads_a_chunked_body() {
    let (url, served) = serve_once(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
         5\r\nhello\r\n6;name=value\r\n world\r\n0\r\n\r\n",
    )
    .await;
    assert_eq!(get(&url, 11).await.unwrap(), b"hello world");
    served.await.unwrap();

    // With neither a length nor chunks, the body runs to the end of the connection.
    let (url, served) = serve_once("HTTP/1.1 200 OK\r\n\r\nto the end").await;
    assert_eq!(get(&url, 1024).await.unwrap(), b"to the end");
    served.await.unwrap();
}
