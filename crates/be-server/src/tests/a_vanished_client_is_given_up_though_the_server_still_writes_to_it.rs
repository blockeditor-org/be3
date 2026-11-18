use super::*;

/// A client that vanished (a phone gone into a tunnel) reads and sends nothing, but what the server
/// writes to it still completes, into a buffer (the kernel's, or Caddy's in front) that nobody
/// reads: its pings and pushes are not taken for it being there, and it is given up at
/// `IDLE_TIMEOUT`.
#[tokio::test(start_paused = true)]
async fn a_vanished_client_is_given_up_though_the_server_still_writes_to_it() {
    let (_client, server) = tokio::io::duplex(16 << 20);
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
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let ping = sink.send(WsMessage::Ping(Vec::new().into())).await;
            let push = sink.send(WsMessage::Binary(vec![7u8; 1 << 10].into())).await;
            if ping.is_err() || push.is_err() {
                return;
            }
        }
    });
    let started = tokio::time::Instant::now();

    assert!(
        next_heard(&mut source, &heard).await.is_none(),
        "a client that sends nothing is given up"
    );

    assert_eq!(
        started.elapsed(),
        IDLE_TIMEOUT,
        "what the server wrote meanwhile, which went into the buffer, kept it no longer"
    );
    writing.abort();
}
