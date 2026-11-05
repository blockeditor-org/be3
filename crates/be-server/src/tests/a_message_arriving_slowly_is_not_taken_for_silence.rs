use super::*;

/// A binary websocket frame as a client sends it, masked.
fn masked_binary(payload: &[u8]) -> Vec<u8> {
    let mask = [1u8, 2, 3, 4];
    let mut frame = vec![0x82, 0x80 | 127];
    frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    frame.extend_from_slice(&mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    frame
}

#[tokio::test(start_paused = true)]
async fn a_message_arriving_slowly_is_not_taken_for_silence() {
    let (mut client, server) = tokio::io::duplex(4 << 20);
    let (server, heard) = Watched::new(server);
    let socket = WebSocketStream::from_raw_socket(
        server,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;
    let (_sink, mut source) = socket.split();
    let payload = vec![7u8; 1 << 20];
    let frame = masked_binary(&payload);
    let reading = tokio::spawn(async move {
        let message = next_heard(&mut source, &heard).await;
        (message, source, heard)
    });
    let started = tokio::time::Instant::now();
    let mut last_written = started;
    for chunk in frame.chunks(frame.len() / 4 + 1) {
        client.write_all(chunk).await.unwrap();
        last_written = tokio::time::Instant::now();
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(IDLE_TIMEOUT / 2).await;
    }
    let (message, mut source, heard) = reading.await.unwrap();
    assert!(
        started.elapsed() > IDLE_TIMEOUT,
        "the message took longer than the idle timeout to arrive"
    );
    assert!(
        matches!(message, Some(Ok(WsMessage::Binary(bytes))) if bytes == payload),
        "a message whose bytes kept coming was read whole"
    );

    assert!(next_heard(&mut source, &heard).await.is_none());
    assert!(
        last_written.elapsed() >= IDLE_TIMEOUT,
        "only a connection that sent nothing for the idle timeout is given up on"
    );
}
