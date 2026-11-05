use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

mod a_get_reads_a_body_its_length_delimits;
mod a_get_reads_a_chunked_body;
mod a_put_sends_its_headers_and_body;
mod an_answer_longer_than_the_limit_is_refused;
mod an_unreachable_server_fails;
mod missing_refused_and_failing_answers_are_told_apart;

/// What a test server was sent.
struct Sent {
    head: String,
    body: Vec<u8>,
}

/// Starts a server on a free local port that answers one request with `answer`, and answers its
/// URL and what it was sent.
async fn serve_once(answer: impl Into<Vec<u8>>) -> (String, tokio::task::JoinHandle<Sent>) {
    let answer = answer.into();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let served = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        let mut chunk = [0u8; 8192];
        let end = loop {
            if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
                break end + 4;
            }
            let read = stream.read(&mut chunk).await.unwrap();
            assert!(read > 0, "the client sent a whole request head");
            received.extend_from_slice(&chunk[..read]);
        };
        let head = String::from_utf8(received[..end].to_vec()).unwrap();
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        let mut body = received[end..].to_vec();
        while body.len() < length {
            let read = stream.read(&mut chunk).await.unwrap();
            assert!(read > 0, "the client sent its whole body");
            body.extend_from_slice(&chunk[..read]);
        }
        // A client that refuses a long answer closes the connection partway through it.
        let _ = stream.write_all(&answer).await;
        let _ = stream.shutdown().await;
        Sent { head, body }
    });
    (url, served)
}
