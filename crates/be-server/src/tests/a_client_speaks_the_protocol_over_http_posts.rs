use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

async fn post(
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

#[tokio::test]
async fn a_client_speaks_the_protocol_over_http_posts() {
    let harness = Harness::start().await;
    let address = harness.url.trim_start_matches("ws://").to_owned();
    let mut stream = tokio::net::TcpStream::connect(&address).await.unwrap();

    let (status, session, frames) = post(&mut stream, None, &[&hello(BUILD)]).await;
    assert_eq!(status, 200);
    assert_eq!(read_hello(&frames[0]).as_deref(), Some(BUILD));
    let session = session.expect("a greeting of the same build opens a session");

    let register = encode(&ClientMessage::Register {
        request: 1,
        email: "http@example.com".into(),
        display_name: "Http".into(),
        password: "a long enough password".into(),
    })
    .unwrap();
    let (_, _, frames) = post(&mut stream, Some(&session), &[&register]).await;
    let response: ServerMessage = be_protocol::decode(frames.last().unwrap()).unwrap();
    assert!(
        matches!(response, ServerMessage::Authenticated { request: 1, .. }),
        "{response:?}"
    );

    let list = encode(&ClientMessage::ListOrganizations { request: 2 }).unwrap();
    let again = encode(&ClientMessage::ListOrganizations { request: 3 }).unwrap();
    let (_, _, frames) = post(&mut stream, Some(&session), &[&list, &again]).await;
    let requests: Vec<Option<u64>> = frames
        .iter()
        .map(|frame| be_protocol::decode::<ServerMessage>(frame).unwrap())
        .filter(|response| matches!(response, ServerMessage::Organizations { .. }))
        .map(|response| response.request())
        .collect();
    assert_eq!(
        requests,
        [Some(2), Some(3)],
        "two frames in one post were not both answered, in order, signed in"
    );

    let (status, _, _) = post(&mut stream, Some("not-a-session"), &[&list]).await;
    assert_eq!(status, 404);

    let mut stranger = tokio::net::TcpStream::connect(&address).await.unwrap();
    let (status, session, frames) = post(&mut stranger, None, &[&hello("another build")]).await;
    assert_eq!(status, 200);
    assert_eq!(read_hello(&frames[0]).as_deref(), Some(BUILD));
    assert_eq!(session, None, "another build was given a session");

    let ServerMessage::Authenticated { token, .. } = response else {
        panic!("not authenticated")
    };
    let authorization = format!("Bearer {token}");
    let mut proxy = tokio::net::TcpStream::connect(&address).await.unwrap();
    let (_, session, _) =
        post_authorized(&mut proxy, None, &[&hello(BUILD)], Some(&authorization)).await;
    let session = session.unwrap();
    let authenticate = encode(&ClientMessage::AuthenticateHeader { request: 4 }).unwrap();
    let (_, _, frames) = post_authorized(
        &mut proxy,
        Some(&session),
        &[&authenticate],
        Some(&authorization),
    )
    .await;
    assert!(
        matches!(be_protocol::decode::<ServerMessage>(frames.last().unwrap()).unwrap(), ServerMessage::Authenticated { token, .. } if token.is_empty())
    );
    let (status, _, _) =
        post_authorized(&mut proxy, Some(&session), &[&list], Some(&authorization)).await;
    assert_eq!(status, 200);
    let (status, _, _) = post(&mut proxy, Some(&session), &[&list]).await;
    assert_eq!(
        status, 401,
        "a session id alone must not replace the injected account token"
    );
    harness.stop().await;
}
