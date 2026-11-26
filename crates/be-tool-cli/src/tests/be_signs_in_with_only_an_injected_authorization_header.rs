use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

/// Where `needle` first ends in `haystack`.
fn end_of(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| at + needle.len())
}

/// A proxy in front of `server` that adds `Authorization: Bearer {token}` to every request it
/// forwards, as a cloud environment's proxy injects an account token; answers its address.
async fn injecting_proxy(server: std::net::SocketAddr, token: String) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((client, _)) = listener.accept().await {
            let header = format!("Authorization: Bearer {token}\r\n");
            tokio::spawn(async move {
                let Ok(upstream) = tokio::net::TcpStream::connect(server).await else {
                    return;
                };
                let (mut from_client, mut to_client) = client.into_split();
                let (mut from_server, mut to_server) = upstream.into_split();
                tokio::spawn(async move {
                    let _ = tokio::io::copy(&mut from_server, &mut to_client).await;
                });
                let mut buffer = Vec::new();
                let mut chunk = [0; 16 * 1024];
                loop {
                    let head = loop {
                        if let Some(end) = end_of(&buffer, b"\r\n\r\n") {
                            break end;
                        }
                        match from_client.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                        }
                    };
                    let text = String::from_utf8_lossy(&buffer[..head]).to_ascii_lowercase();
                    let length = text
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|length| length.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    let line = end_of(&buffer, b"\r\n").unwrap();
                    let mut request = buffer[..line].to_vec();
                    request.extend_from_slice(header.as_bytes());
                    request.extend_from_slice(&buffer[line..head]);
                    buffer.drain(..head);
                    while buffer.len() < length {
                        match from_client.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                        }
                    }
                    request.extend(buffer.drain(..length));
                    if to_server.write_all(&request).await.is_err() {
                        return;
                    }
                    if text.contains("upgrade: websocket") {
                        let _ = to_server.write_all(&buffer).await;
                        let _ = tokio::io::copy(&mut from_client, &mut to_server).await;
                        return;
                    }
                }
            });
        }
    });
    address
}

/// `be` with proxy credentials holds no token: it greets the server with none, and the account
/// token the proxy injects into the request's Authorization header signs it in. Without that
/// header it is refused.
#[tokio::test]
async fn be_signs_in_with_only_an_injected_authorization_header() {
    let harness = Harness::start().await;
    let owner = Tracker::new(harness.owner().await, sources());
    let (mut settings, account, _) = agent(&harness, &owner, None).await;
    let token = std::mem::take(&mut settings.token);

    let refused = crate::session::connect(&settings).await;
    assert!(
        refused.is_err(),
        "be signed in with neither a token nor an injected header"
    );

    let proxy = injecting_proxy(harness.address, token).await;
    settings.server = format!("http://{proxy}");
    let peer = crate::session::connect(&settings)
        .await
        .expect("the injected header signs be in");
    assert_eq!(peer.account(), account);
    harness.stop().await;
}
