use super::*;

/// Reads one answer's head and body, leaving the connection at the next answer; its status.
async fn answered(stream: &mut tokio::net::TcpStream) -> String {
    let mut received = Vec::new();
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the server closed the connection");
        received.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&received[..end]).to_string();
    let length: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap_or(0);
    while received.len() < end + length {
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the server closed the connection");
        received.extend_from_slice(&chunk[..read]);
    }
    head.split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned()
}

fn upgrade(forwarded: &str) -> String {
    format!(
        "GET / HTTP/1.1\r\nHost: test\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nX-Forwarded-For: {forwarded}\r\n\r\n"
    )
}

async fn upgraded(harness: &Harness, forwarded: &str) -> (tokio::net::TcpStream, String) {
    let mut stream = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    stream
        .write_all(upgrade(forwarded).as_bytes())
        .await
        .unwrap();
    let status = answered(&mut stream).await;
    (stream, status)
}

/// A proxy pools its connections, so one carries one client's requests and then another's: a
/// websocket opened by the second is counted for the second, never in the first's places.
#[tokio::test]
async fn a_proxy_connection_takes_each_requests_places_for_the_client_it_forwards() {
    let harness = Harness::start().await;
    let (first, second) = ("198.51.100.20", "198.51.100.21");
    let mut open = Vec::new();
    for _ in 1..crate::MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS {
        let (stream, status) = upgraded(&harness, second).await;
        assert_eq!(status, "101");
        open.push(stream);
    }

    let mut pooled = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    pooled
        .write_all(
            format!(
                "POST /api/be/http HTTP/1.1\r\nHost: test\r\nBe3-Session: none\r\nX-Forwarded-For: {first}\r\nContent-Length: 0\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    assert_eq!(
        answered(&mut pooled).await,
        "404",
        "the post is answered and the connection kept"
    );
    pooled.write_all(upgrade(second).as_bytes()).await.unwrap();
    assert_eq!(answered(&mut pooled).await, "101");

    let (_, status) = upgraded(&harness, second).await;
    assert_eq!(
        status, "429",
        "the websocket on the pooled connection holds one of the second client's places"
    );
    let (_other, status) = upgraded(&harness, first).await;
    assert_eq!(status, "101", "the first client holds none of it");
    drop((open, pooled));
    harness.stop().await;
}
