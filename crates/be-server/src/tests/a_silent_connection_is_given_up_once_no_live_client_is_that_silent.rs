use super::*;

/// A ping as a client sends it: a masked, empty binary frame.
fn masked_ping() -> Vec<u8> {
    vec![0x82, 0x80, 1, 2, 3, 4]
}

#[tokio::test(start_paused = true)]
async fn a_silent_connection_is_given_up_once_no_live_client_is_that_silent() {
    let (mut client, server) = tokio::io::duplex(1 << 16);
    let (server, heard) = Watched::new(server);
    let socket = WebSocketStream::from_raw_socket(
        server,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;
    let (_sink, mut source) = socket.split();
    let reading = tokio::spawn(async move {
        let mut messages = 0;
        while next_heard(&mut source, &heard).await.is_some() {
            messages += 1;
        }
        messages
    });

    // A live client: one ping each PING_INTERVAL, which the server reads.
    for _ in 0..4 {
        tokio::time::advance(be_block_protocol::PING_INTERVAL).await;
        client.write_all(&masked_ping()).await.unwrap();
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }
    let silent = tokio::time::Instant::now();
    let messages = reading.await.unwrap();

    assert_eq!(messages, 4, "a client that pings is read for as long as it pings");
    assert_eq!(
        silent.elapsed(),
        IDLE_TIMEOUT,
        "a connection gone silent is given up, and leaves its sessions, at IDLE_TIMEOUT"
    );
    assert!(
        IDLE_TIMEOUT < std::time::Duration::from_secs(60),
        "a silent holder gives up its lease in under a minute, not the 90 seconds it once took"
    );
}
