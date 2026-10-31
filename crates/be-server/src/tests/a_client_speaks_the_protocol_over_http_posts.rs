use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

pub(super) async fn post(
    stream: &mut tokio::net::TcpStream,
    session: Option<&str>,
    frames: &[&[u8]],
) -> (u16, Option<String>, Vec<Vec<u8>>) {
    post_authorized(stream, session, frames, None).await
}

async fn post_authorized(
    stream: &mut tokio::net::TcpStream,
    session: Option<&str>,
    frames: &[&[u8]],
    authorization: Option<&str>,
) -> (u16, Option<String>, Vec<Vec<u8>>) {
    let mut body = Vec::new();
    for frame in frames {
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend_from_slice(frame);
    }
    let mut head = format!(
        "POST /api/be/http HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(session) = session {
        head.push_str(&format!("Be3-Session: {session}\r\n"));
    }
    if let Some(authorization) = authorization {
        head.push_str(&format!("Authorization: {authorization}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(&body).await.unwrap();

    let mut received = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the server closed the connection");
        received.extend_from_slice(&chunk[..read]);
    };
    let text = String::from_utf8_lossy(&received[..end]).to_string();
    let status = text.split_whitespace().nth(1).unwrap().parse().unwrap();
    let header = |name: &str| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length").unwrap().parse().unwrap();
    let mut body = received[end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the server closed the connection");
        body.extend_from_slice(&chunk[..read]);
    }
    let mut frames = Vec::new();
    let mut rest = &body[..];
    while status == 200 && rest.len() >= 4 {
        let size = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
        frames.push(rest[4..4 + size].to_vec());
        rest = &rest[4 + size..];
    }
    (status, header("be3-session"), frames)
}

pub(super) struct Posted {
    pub(super) peer: Peer,
    outbound: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
}

impl Posted {
    pub(super) fn new() -> Self {
        let (sending, outbound) = tokio::sync::mpsc::unbounded_channel();
        let peer = Peer::new(be_ops::Side::Guest, move |frame| {
            let _ = sending.send(frame);
        });
        Self { peer, outbound }
    }

    pub(super) fn frames(&mut self) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| self.outbound.try_recv().ok()).collect()
    }

    pub(super) fn heard(&self, frames: &[Vec<u8>]) {
        for frame in frames {
            self.peer.receive(frame).unwrap();
        }
    }
}

fn refs(frames: &[Vec<u8>]) -> Vec<&[u8]> {
    frames.iter().map(Vec::as_slice).collect()
}

#[tokio::test]
async fn a_client_speaks_the_protocol_over_http_posts() {
    let harness = Harness::start().await;
    let address = harness.url.trim_start_matches("ws://").to_owned();
    let mut stream = tokio::net::TcpStream::connect(&address).await.unwrap();

    let (status, session, frames) = post(&mut stream, None, &[&hello(BUILD)]).await;
    assert_eq!(status, 200);
    assert_eq!(read_hello(&frames[0]).as_deref(), Some(BUILD));
    let session = session.expect("a greeting of the same build opens a session");

    let (_, token) = ServerStore::open(&harness.directory)
        .unwrap()
        .register("http@example.com", "Http", "a long enough password")
        .await
        .unwrap();
    let mut posted = Posted::new();
    let mut registering = client::account::authenticate(&posted.peer, token);
    let sent = posted.frames();
    let (_, _, frames) = post(&mut stream, Some(&session), &refs(&sent)).await;
    posted.heard(&frames);
    let std::task::Poll::Ready(Ok(response)) = poll(&mut registering) else {
        panic!("the post was not answered");
    };
    assert!(matches!(response, Ok(Authenticated { .. })), "{response:?}");

    let mut list = client::organizations::list_organizations(&posted.peer);
    let mut again = client::organizations::list_organizations(&posted.peer);
    let sent = posted.frames();
    let (_, _, frames) = post(&mut stream, Some(&session), &refs(&sent)).await;
    let answered: Vec<be_ops::OpId> = frames
        .iter()
        .filter_map(|frame| match be_ops::decode_frame::<be_ops::Frame>(frame) {
            Ok(be_ops::Frame::Complete { op, .. }) => Some(op),
            _ => None,
        })
        .collect();
    assert_eq!(
        answered,
        [list.op(), again.op()],
        "two frames in one post were not both answered, in order"
    );
    posted.heard(&frames);
    assert!(
        matches!(poll(&mut list), std::task::Poll::Ready(Ok(Ok(_))))
            && matches!(poll(&mut again), std::task::Poll::Ready(Ok(Ok(_)))),
        "two frames in one post were not both answered signed in"
    );

    let listing = Posted::new();
    let _listed = client::organizations::list_organizations(&listing.peer);
    let mut listing = listing;
    let list = listing.frames();
    let (status, _, _) = post(&mut stream, Some("not-a-session"), &refs(&list)).await;
    assert_eq!(status, 404);

    let mut stranger = tokio::net::TcpStream::connect(&address).await.unwrap();
    let (status, session, frames) = post(&mut stranger, None, &[&hello("another build")]).await;
    assert_eq!(status, 200);
    assert_eq!(read_hello(&frames[0]).as_deref(), Some(BUILD));
    assert_eq!(session, None, "another build was given a session");

    let Ok(Authenticated { token, .. }) = response else {
        panic!("not authenticated")
    };
    let authorization = format!("Bearer {token}");
    let mut proxy = tokio::net::TcpStream::connect(&address).await.unwrap();
    let (_, session, _) =
        post_authorized(&mut proxy, None, &[&hello(BUILD)], Some(&authorization)).await;
    let session = session.unwrap();
    let mut proxied = Posted::new();
    let mut authenticating = client::account::authenticate_header(&proxied.peer);
    let sent = proxied.frames();
    let (_, _, frames) = post_authorized(
        &mut proxy,
        Some(&session),
        &refs(&sent),
        Some(&authorization),
    )
    .await;
    proxied.heard(&frames);
    assert!(matches!(
        poll(&mut authenticating),
        std::task::Poll::Ready(Ok(Ok(Authenticated { token, .. }))) if token.is_empty()
    ));
    let _listed = client::organizations::list_organizations(&proxied.peer);
    let list = proxied.frames();
    let (status, _, _) = post_authorized(
        &mut proxy,
        Some(&session),
        &refs(&list),
        Some(&authorization),
    )
    .await;
    assert_eq!(status, 200);
    let (status, _, _) = post(&mut proxy, Some(&session), &refs(&list)).await;
    assert_eq!(
        status, 401,
        "a session id alone must not replace the injected account token"
    );
    harness.stop().await;
}
