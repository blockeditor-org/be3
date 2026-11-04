use super::*;

async fn status(stream: &mut tokio::net::TcpStream) -> String {
    let mut received = Vec::new();
    let mut chunk = [0u8; 1024];
    while !received.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        received.extend_from_slice(&chunk[..read]);
    }
    String::from_utf8_lossy(&received)
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned()
}

async fn upgrade(harness: &Harness, forwarded: &str) -> (tokio::net::TcpStream, String) {
    let mut stream = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    stream
        .write_all(
            format!(
                "GET / HTTP/1.1\r\nHost: test\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nX-Forwarded-For: {forwarded}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let status = status(&mut stream).await;
    (stream, status)
}

async fn greet_over_http(harness: &Harness, forwarded: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    let hello = hello(BUILD);
    let mut body = (hello.len() as u32).to_be_bytes().to_vec();
    body.extend_from_slice(&hello);
    stream
        .write_all(
            format!(
                "POST /api/be/http HTTP/1.1\r\nHost: test\r\nConnection: close\r\nX-Forwarded-For: {forwarded}\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    stream.write_all(&body).await.unwrap();
    status(&mut stream).await
}

#[tokio::test]
async fn one_address_holds_only_so_many_connections_and_http_sessions() {
    let harness = Harness::start().await;
    let mut open = Vec::new();
    for _ in 0..MAX_CONNECTIONS_PER_ADDRESS {
        let (stream, status) = upgrade(&harness, "198.51.100.1").await;
        assert_eq!(status, "101");
        open.push(stream);
    }
    let (_, status) = upgrade(&harness, "198.51.100.1").await;
    assert_eq!(
        status, "429",
        "an address that holds its every place is refused another"
    );
    let (_other, status) = upgrade(&harness, "198.51.100.2").await;
    assert_eq!(status, "101", "another address is not held back by it");

    for _ in 0..http::MAX_HTTP_SESSIONS_PER_ADDRESS {
        assert_eq!(greet_over_http(&harness, "198.51.100.3").await, "200");
    }
    assert_eq!(
        greet_over_http(&harness, "198.51.100.3").await,
        "503",
        "an address holds only so many HTTP sessions, though each post's connection closed"
    );
    assert_eq!(greet_over_http(&harness, "198.51.100.4").await, "200");
    drop(open);
    harness.stop().await;
}
