use super::*;

/// A proxy in front may read a request's body by its `Transfer-Encoding` or by another of its
/// lengths than the server would, and smuggle a second request in it, so the server refuses
/// both rather than pick one.
#[tokio::test]
async fn a_request_with_a_transfer_encoding_or_two_lengths_is_refused() {
    let harness = Harness::start().await;
    let status = |extra: &'static [u8]| {
        let address = harness.address;
        async move {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream
                .write_all(
                    &[
                        format!("GET /auth/login HTTP/1.1\r\nHost: {address}\r\n").as_bytes(),
                        extra,
                        b"Connection: close\r\n\r\n0\r\n\r\n",
                    ]
                    .concat(),
                )
                .await
                .unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.unwrap();
            let response = String::from_utf8(bytes).unwrap();
            response
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_owned()
        }
    };
    assert_eq!(status(b"").await, "200");
    assert_eq!(status(b"Content-Length:  0 \r\n").await, "200");
    assert_eq!(status(b"Transfer-Encoding: chunked\r\n").await, "400");
    assert_eq!(
        status(b"Content-Length: 0\r\nContent-Length: 5\r\n").await,
        "400"
    );
    // A length that would read as none, or as a number no proxy reads it as.
    for odd in [
        b"Content-Length: 5\xff\r\n".as_slice(),
        "Content-Length: 5\u{a0}\r\n".as_bytes(),
        b"Content-Length: +5\r\n",
        b"Content-Length: \r\n",
    ] {
        assert_eq!(status(odd).await, "400", "{}", String::from_utf8_lossy(odd));
    }
}
