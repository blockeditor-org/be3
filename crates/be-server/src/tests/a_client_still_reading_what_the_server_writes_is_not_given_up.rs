use super::*;

use tokio::io::AsyncReadExt;

/// A client on a slow link may spend longer than `IDLE_TIMEOUT` taking in what the server writes,
/// sending nothing meanwhile (its automatic Pong waits behind it): while the server's writes go
/// out it is not silent, and only once they stop going out is it given up.
#[tokio::test(start_paused = true)]
async fn a_client_still_reading_what_the_server_writes_is_not_given_up() {
    let (mut client, server) = tokio::io::duplex(64 << 10);
    let (server, heard) = Watched::new(server);
    let socket = WebSocketStream::from_raw_socket(
        server,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;
    let (mut sink, mut source) = socket.split();
    let writing = tokio::spawn(async move {
        loop {
            if sink
                .send(WsMessage::Binary(vec![7u8; 32 << 10].into()))
                .await
                .is_err()
            {
                return;
            }
        }
    });
    let listening = tokio::spawn(async move { next_heard(&mut source, &heard).await.is_none() });

    // Three idle timeouts of a client reading a little every few seconds, and sending nothing.
    let started = tokio::time::Instant::now();
    let mut buffer = vec![0u8; 16 << 10];
    while started.elapsed() < 3 * IDLE_TIMEOUT {
        tokio::time::advance(std::time::Duration::from_secs(5)).await;
        let _ = client.read(&mut buffer).await.unwrap();
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }
    assert!(
        !listening.is_finished(),
        "a client still reading what the server writes is not given up"
    );

    // It stops reading: the writes stop going out, and it is given up IDLE_TIMEOUT later.
    let stopped = tokio::time::Instant::now();
    assert!(
        listening.await.unwrap(),
        "a client whose writes no longer go out is given up"
    );
    assert!(stopped.elapsed() <= IDLE_TIMEOUT + std::time::Duration::from_secs(5));
    writing.abort();
}
