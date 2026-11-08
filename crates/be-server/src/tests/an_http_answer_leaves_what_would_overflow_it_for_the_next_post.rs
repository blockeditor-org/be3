use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

/// Posts `frames` and returns the frames answered and whether the server said it has more.
async fn post(
    stream: &mut tokio::net::TcpStream,
    session: &str,
    frames: &[Vec<u8>],
) -> (Vec<Vec<u8>>, bool) {
    let mut body = Vec::new();
    for frame in frames {
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend_from_slice(frame);
    }
    stream
        .write_all(
            format!(
                "POST /api/be/http HTTP/1.1\r\nHost: test\r\nBe3-Session: {session}\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    stream.write_all(&body).await.unwrap();
    let mut received = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let end = loop {
        if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the server closed the connection");
        received.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&received[..end]).to_ascii_lowercase();
    assert!(head.starts_with("http/1.1 200"), "{head}");
    let length: usize = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let more = head.lines().any(|line| line.starts_with("be3-more:"));
    let mut body = received[end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the server closed the connection");
        body.extend_from_slice(&chunk[..read]);
    }
    let mut frames = Vec::new();
    let mut rest = &body[..];
    while rest.len() >= 4 {
        let size = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
        frames.push(rest[4..4 + size].to_vec());
        rest = &rest[4 + size..];
    }
    (frames, more)
}

fn answered(frames: &[Vec<u8>]) -> usize {
    frames
        .iter()
        .filter(|frame| {
            matches!(
                be_wasm_ops::decode_frame::<be_wasm_ops::Frame>(frame),
                Ok(be_wasm_ops::Frame::Complete { .. })
            )
        })
        .count()
}

#[tokio::test]
async fn an_http_answer_leaves_what_would_overflow_it_for_the_next_post() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("large@example.com").await;
    let organization = owner.organization("large").await;
    let author = Author::new(account);
    let block = owner.create_block(BlockParent::Root).await;
    let (commit, mut chunks) = author.compose(b"large objects\n", 1_000, None);
    let large: Vec<Hash> = (0..4u8)
        .map(|fill| author.store.put(&vec![fill; 3 << 20]).unwrap())
        .collect();
    owner
        .upload(&author.store, &author.objects_of(commit, &chunks))
        .await;
    // One at a time: together they would not fit in one frame.
    for hash in &large {
        owner.upload(&author.store, &[*hash]).await;
    }
    chunks.extend_from_slice(&large);
    let response = owner
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
                None,
                Vec::new(),
                objects_change(&[], chunks),
                1_000,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(response.is_ok(), "{response:?}");

    let mut stream = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    let (_, session, _) =
        a_client_speaks_the_protocol_over_http_posts::post(&mut stream, None, &[&hello(BUILD)])
            .await;
    let session = session.unwrap();
    let mut posted = a_client_speaks_the_protocol_over_http_posts::Posted::new();
    let token = owner.token.clone();
    let _signed_in = client::account::authenticate(&posted.peer, Some(token));
    let _opened = client::organizations::open_organization(&posted.peer, organization);
    let sent = posted.frames();
    let (frames, _) = post(&mut stream, &session, &sent).await;
    posted.heard(&frames);

    // Each read answers the head's objects that fit in half a frame, a large one among them.
    let mut reads: Vec<_> = large
        .iter()
        .map(|_| client::blocks::read_contents(&posted.peer, vec![block], Vec::new()))
        .collect();
    let sent = posted.frames();
    let (mut frames, more) = post(&mut stream, &session, &sent).await;
    assert!(
        answered(&frames) < large.len() && more,
        "one post's answers stop once they fill a reply, and say there is more"
    );
    let mut posts = 1;
    let mut more = more;
    while more {
        let (next, further) = post(&mut stream, &session, &[]).await;
        frames.extend(next);
        more = further;
        posts += 1;
        assert!(posts <= large.len(), "every post answers something");
    }
    assert_eq!(answered(&frames), large.len());
    posted.heard(&frames);
    for read in &mut reads {
        let std::task::Poll::Ready(Ok(Ok(contents))) = poll(read) else {
            panic!("a read was not answered");
        };
        assert!(
            contents.objects.contains(&vec![0; 3 << 20]),
            "a read was not answered with the large object"
        );
    }
    harness.stop().await;
}
