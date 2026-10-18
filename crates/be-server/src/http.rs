use std::{
    collections::HashMap,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use be_protocol::{MAX_FRAME_BYTES, encode, read_hello};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    sync::Mutex,
};
use uuid::Uuid;

use crate::{Client, HANDSHAKE_TIMEOUT, IDLE_TIMEOUT, ServerError, Services, greeting};

const PATH_SUFFIX: &str = "/http";
const SESSION_HEADER: &str = "be3-session";
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 2 * MAX_FRAME_BYTES;
const MAX_HEADERS: usize = 64;

#[derive(Default)]
pub(crate) struct Sessions {
    open: std::sync::Mutex<HashMap<String, Arc<Mutex<Session>>>>,
}

struct Session {
    client: Option<Client>,
    heard: Instant,
}

impl Sessions {
    fn insert(&self, client: Client) -> String {
        let id = Uuid::new_v4().simple().to_string();
        let session = Session {
            client: Some(client),
            heard: Instant::now(),
        };
        self.open
            .lock()
            .unwrap()
            .insert(id.clone(), Arc::new(Mutex::new(session)));
        id
    }

    fn get(&self, id: &str) -> Option<Arc<Mutex<Session>>> {
        self.open.lock().unwrap().get(id).cloned()
    }

    async fn sweep(&self) {
        let open: Vec<(String, Arc<Mutex<Session>>)> = self
            .open
            .lock()
            .unwrap()
            .iter()
            .map(|(id, session)| (id.clone(), Arc::clone(session)))
            .collect();
        for (id, session) in open {
            let Ok(mut held) = session.try_lock() else {
                continue;
            };
            if held.heard.elapsed() > IDLE_TIMEOUT {
                self.open.lock().unwrap().remove(&id);
                if let Some(client) = held.client.take() {
                    client.close().await;
                }
            }
        }
    }

    pub(crate) async fn close_all(&self) {
        let open: Vec<Arc<Mutex<Session>>> = self
            .open
            .lock()
            .unwrap()
            .drain()
            .map(|(_, session)| session)
            .collect();
        for session in open {
            if let Some(client) = session.lock().await.client.take() {
                client.close().await;
            }
        }
    }
}

pub(crate) async fn route(
    mut stream: TcpStream,
    services: &Services,
) -> Result<Option<Prefixed>, ServerError> {
    let mut buffer = Vec::new();
    let read = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_head(&mut stream, &mut buffer))
        .await
        .map_err(|_| ServerError::Socket("the request never finished its head".into()))??;
    if !read {
        return Ok(None);
    }
    let head = Head::parse(&buffer)?;
    if head.method == "POST"
        && head
            .path
            .split('?')
            .next()
            .unwrap_or_default()
            .ends_with(PATH_SUFFIX)
    {
        serve(stream, buffer, services).await?;
        return Ok(None);
    }
    Ok(Some(Prefixed {
        prefix: buffer,
        position: 0,
        stream,
    }))
}

async fn serve(
    mut stream: TcpStream,
    mut buffer: Vec<u8>,
    services: &Services,
) -> Result<(), ServerError> {
    loop {
        let head = Head::parse(&buffer)?;
        if head.length > MAX_BODY_BYTES {
            return respond(&mut stream, 413, None, b"the frame is too large").await;
        }
        let mut body = buffer.split_off(head.size);
        while body.len() < head.length {
            let mut chunk = vec![0; head.length - body.len()];
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                return Ok(());
            }
            body.extend_from_slice(&chunk[..read]);
        }
        buffer = body.split_off(head.length);
        services.http.sweep().await;
        let answer = answer(
            head.session.as_deref(),
            head.authorization.as_deref(),
            &body,
            services,
        )
        .await?;
        respond(
            &mut stream,
            answer.status,
            answer.session.as_deref(),
            &answer.body,
        )
        .await?;
        if head.close || !answer.keep {
            return Ok(());
        }
        match tokio::time::timeout(IDLE_TIMEOUT, read_head(&mut stream, &mut buffer)).await {
            Ok(Ok(true)) => {}
            Ok(Err(error)) => return Err(error),
            Ok(Ok(false)) | Err(_) => return Ok(()),
        }
    }
}

struct Answer {
    status: u16,
    session: Option<String>,
    body: Vec<u8>,
    keep: bool,
}

async fn answer(
    session: Option<&str>,
    authorization: Option<&str>,
    body: &[u8],
    services: &Services,
) -> Result<Answer, ServerError> {
    let Some(frames) = unframed(body) else {
        return Ok(Answer {
            status: 400,
            session: None,
            body: b"the body is not a list of frames".to_vec(),
            keep: false,
        });
    };
    let Some(id) = session else {
        let theirs = frames.first().and_then(|frame| read_hello(frame));
        let (ours, compatible) = greeting(theirs.as_deref())?;
        let session = match compatible {
            true => {
                let mut client = Client::open(services).await;
                client.connection.header_token = crate::bearer(authorization);
                Some(services.http.insert(client))
            }
            false => None,
        };
        return Ok(Answer {
            status: 200,
            session,
            body: framed(&[ours]),
            keep: compatible,
        });
    };
    let gone = Answer {
        status: 404,
        session: None,
        body: b"no such session".to_vec(),
        keep: true,
    };
    let Some(session) = services.http.get(id) else {
        return Ok(gone);
    };
    let mut held = session.lock().await;
    held.heard = Instant::now();
    let Some(client) = held.client.as_mut() else {
        return Ok(gone);
    };
    if client.connection.header_token != crate::bearer(authorization) {
        return Ok(Answer {
            status: 401,
            session: None,
            body: b"account authorization changed".to_vec(),
            keep: false,
        });
    }
    let mut answers = Vec::new();
    for frame in frames {
        for message in client.answer(frame).await {
            answers.push(encode(&message).map_err(|_| ServerError::Corrupt)?);
        }
    }
    Ok(Answer {
        status: 200,
        session: None,
        body: framed(&answers),
        keep: true,
    })
}

fn unframed(body: &[u8]) -> Option<Vec<&[u8]>> {
    let mut frames = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let size = u32::from_be_bytes(rest.get(..4)?.try_into().ok()?) as usize;
        frames.push(rest.get(4..4 + size)?);
        rest = &rest[4 + size..];
    }
    Some(frames)
}

fn framed(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for frame in frames {
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend_from_slice(frame);
    }
    body
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    session: Option<&str>,
    body: &[u8],
) -> Result<(), ServerError> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/octet-stream\r\nCache-Control: no-store\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(session) = session {
        head.push_str(&format!("{SESSION_HEADER}: {session}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_head(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<bool, ServerError> {
    let mut chunk = [0u8; 4096];
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
        if buffer.len() > MAX_HEAD_BYTES {
            return Err(ServerError::Socket("the request head is too long".into()));
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(false);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(true)
}

struct Head {
    method: String,
    path: String,
    size: usize,
    length: usize,
    session: Option<String>,
    close: bool,
    authorization: Option<String>,
}

impl Head {
    fn parse(buffer: &[u8]) -> Result<Self, ServerError> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut request = httparse::Request::new(&mut headers);
        let size = match request.parse(buffer) {
            Ok(httparse::Status::Complete(size)) => size,
            _ => return Err(ServerError::Socket("the request head is malformed".into())),
        };
        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|header| header.name.eq_ignore_ascii_case(name))
                .and_then(|header| std::str::from_utf8(header.value).ok())
                .map(str::trim)
        };
        let length = match header("content-length") {
            Some(value) => value
                .parse()
                .map_err(|_| ServerError::Socket("the content length is not a number".into()))?,
            None => 0,
        };
        let session = header(SESSION_HEADER).map(str::to_owned);
        let close = header("connection").is_some_and(|value| value.eq_ignore_ascii_case("close"));
        Ok(Self {
            method: request.method.unwrap_or_default().to_owned(),
            path: request.path.unwrap_or_default().to_owned(),
            size,
            length,
            session,
            close,
            authorization: header("authorization").map(str::to_owned),
        })
    }
}

pub(crate) struct Prefixed {
    prefix: Vec<u8>,
    position: usize,
    stream: TcpStream,
}

impl AsyncRead for Prefixed {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.position < self.prefix.len() {
            let left = &self.prefix[self.position..];
            let count = left.len().min(buffer.remaining());
            buffer.put_slice(&left[..count]);
            self.position += count;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.stream).poll_read(context, buffer)
    }
}

impl AsyncWrite for Prefixed {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(context, buffer)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}
