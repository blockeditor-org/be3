use std::{
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    sync::mpsc,
};

use super::{Frame, TUNNEL_WAIT, proxied_tls, proxy_for, tunnel};
use crate::transport::{framed, frames};

const SESSION_HEADER: &str = "be3-session";
/// Set on an answer when the server has more to send, or frames still to carry out; the client
/// posts again at once.
const MORE_HEADER: &str = "be3-more";
/// Longest a post may go with no bytes moving either way before it fails. A large post or answer
/// on a slow link takes as long as it takes, as long as its bytes keep moving.
const QUIET_WAIT: Duration = Duration::from_secs(120);
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
const BATCH_BYTES: usize = be_block_protocol::MAX_FRAME_BYTES;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

/// When bytes last moved on a client's connection, either way.
#[derive(Clone)]
struct Moved(Arc<Mutex<tokio::time::Instant>>);

impl Moved {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(tokio::time::Instant::now())))
    }

    fn now(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = tokio::time::Instant::now();
    }

    fn last(&self) -> tokio::time::Instant {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A connection that notes in its `Moved` each time bytes are read or written.
struct Moving {
    inner: Box<dyn Io>,
    moved: Moved,
}

impl AsyncRead for Moving {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(context, buffer);
        if buffer.filled().len() > before {
            self.moved.now();
        }
        polled
    }
}

impl AsyncWrite for Moving {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let polled = Pin::new(&mut self.inner).poll_write(context, buffer);
        if matches!(polled, Poll::Ready(Ok(written)) if written > 0) {
            self.moved.now();
        }
        polled
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

/// Runs `exchange`, failing it once no bytes have moved for `QUIET_WAIT`.
async fn while_moving<T>(
    exchange: impl Future<Output = Result<T, String>>,
    moved: &Moved,
) -> Result<T, String> {
    moved.now();
    let mut exchange = std::pin::pin!(exchange);
    loop {
        tokio::select! {
            done = &mut exchange => return done,
            () = tokio::time::sleep_until(moved.last() + QUIET_WAIT) => {
                if moved.last() + QUIET_WAIT <= tokio::time::Instant::now() {
                    return Err("the server stopped answering".to_owned());
                }
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub(super) async fn connect(
    url: &str,
) -> Result<
    (
        mpsc::UnboundedSender<Vec<u8>>,
        mpsc::UnboundedReceiver<Frame>,
    ),
    String,
> {
    let mut client = Client {
        target: Target::parse(url)?,
        authorization: crate::authorization(url),
        stream: None,
        session: None,
        moved: Moved::new(),
    };
    let stream = client.target.open().await?;
    client.stream = Some(client.moving(stream));
    let (posts, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let (incoming, frames) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut more = false;
        loop {
            let mut body = match more {
                true => Vec::new(),
                false => match outgoing.recv().await {
                    Some(first) => framed(&first),
                    None => return,
                },
            };
            while body.len() < BATCH_BYTES {
                let Ok(next) = outgoing.try_recv() else {
                    break;
                };
                body.extend_from_slice(&framed(&next));
            }
            let moved = client.moved.clone();
            let answered = while_moving(client.post(&body), &moved).await;
            match answered {
                Ok((answer, further)) => {
                    more = further;
                    for frame in answer {
                        if incoming.send(Frame::Binary(frame)).is_err() {
                            return;
                        }
                    }
                }
                Err(error) => {
                    let _ = incoming.send(Frame::Failed(error));
                    return;
                }
            }
        }
    });
    Ok((posts, frames))
}

/// Posts `body` to `url` once, on a connection of its own, and answers the frames of the answer.
pub(crate) async fn post_once(url: &str, body: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut client = Client {
        target: Target::parse(url)?,
        authorization: crate::authorization(url),
        stream: None,
        session: None,
        moved: Moved::new(),
    };
    let moved = client.moved.clone();
    let (frames, _) = while_moving(client.post(body), &moved).await?;
    Ok(frames)
}

struct Target {
    secure: bool,
    host: String,
    port: u16,
    path: String,
}

impl Target {
    fn parse(url: &str) -> Result<Self, String> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| format!("{url} has no scheme"))?;
        let secure = match scheme {
            "https" => true,
            "http" => false,
            other => return Err(format!("{other} is not http or https")),
        };
        let (authority, path) = match rest.find('/') {
            Some(slash) => (&rest[..slash], &rest[slash..]),
            None => (rest, "/"),
        };
        let default = if secure { 443 } else { 80 };
        let (host, port) = match authority.strip_prefix('[') {
            Some(bracketed) => {
                let (host, after) = bracketed
                    .split_once(']')
                    .ok_or_else(|| format!("{authority} is not a valid host"))?;
                (host, after.strip_prefix(':'))
            }
            None => match authority.rsplit_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (authority, None),
            },
        };
        let port = match port {
            Some(port) => port
                .parse()
                .map_err(|_| format!("{authority} has no valid port"))?,
            None => default,
        };
        Ok(Self {
            secure,
            host: host.to_owned(),
            port,
            path: path.to_owned(),
        })
    }

    fn host_header(&self) -> String {
        let host = match self.host.contains(':') {
            true => format!("[{}]", self.host),
            false => self.host.clone(),
        };
        match (self.secure, self.port) {
            (true, 443) | (false, 80) => host,
            (_, port) => format!("{host}:{port}"),
        }
    }

    async fn open(&self) -> Result<Box<dyn Io>, String> {
        let stream = match proxy_for(&self.host).filter(|_| self.secure) {
            Some(proxy) => tokio::time::timeout(TUNNEL_WAIT, tunnel(&proxy, &self.host, self.port))
                .await
                .map_err(|_| format!("the proxy at {proxy} did not open a tunnel in time"))??,
            None => TcpStream::connect((self.host.as_str(), self.port))
                .await
                .map_err(|error| format!("could not reach {}: {error}", self.host))?,
        };
        let _ = stream.set_nodelay(true);
        if !self.secure {
            return Ok(Box::new(stream));
        }
        let name = rustls::pki_types::ServerName::try_from(self.host.clone())
            .map_err(|_| format!("{} is not a valid server name", self.host))?;
        let tls = tokio_rustls::TlsConnector::from(Arc::new(proxied_tls()?))
            .connect(name, stream)
            .await
            .map_err(|error| format!("could not open TLS to {}: {error}", self.host))?;
        Ok(Box::new(tls))
    }
}

struct Client {
    target: Target,
    authorization: Option<String>,
    stream: Option<Box<dyn Io>>,
    session: Option<String>,
    moved: Moved,
}

struct Response {
    status: u16,
    session: Option<String>,
    more: bool,
    close: bool,
    body: Vec<u8>,
}

impl Client {
    fn moving(&self, inner: Box<dyn Io>) -> Box<dyn Io> {
        Box::new(Moving {
            inner,
            moved: self.moved.clone(),
        })
    }

    /// Posts `body` and returns the frames answered, and whether the server has more.
    async fn post(&mut self, body: &[u8]) -> Result<(Vec<Vec<u8>>, bool), String> {
        let reused = self.stream.is_some();
        let response = match self.exchange(body).await {
            Err(Stale) if reused => {
                self.stream = None;
                self.exchange(body).await
            }
            other => other,
        }
        .map_err(|Stale| "the server closed the connection".to_owned())??;
        if response.close {
            self.stream = None;
        }
        if response.status != 200 {
            return Err(format!(
                "the server answered {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            ));
        }
        if let Some(session) = response.session {
            self.session = Some(session);
        }
        Ok((frames(&response.body)?, response.more))
    }

    async fn exchange(&mut self, body: &[u8]) -> Result<Result<Response, String>, Stale> {
        let stream = match self.stream.take() {
            Some(stream) => stream,
            None => match self.target.open().await {
                Ok(stream) => self.moving(stream),
                Err(error) => return Ok(Err(error)),
            },
        };
        let stream = self.stream.insert(stream);
        let mut head = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n",
            self.target.path,
            self.target.host_header(),
            body.len()
        );
        if let Some(authorization) = &self.authorization {
            head.push_str(&format!("Be3-Gateway-Authorization: {authorization}\r\n"));
        }
        if let Some(session) = &self.session {
            head.push_str(&format!("{SESSION_HEADER}: {session}\r\n"));
        }
        head.push_str("\r\n");
        if stream.write_all(head.as_bytes()).await.is_err()
            || stream.write_all(body).await.is_err()
            || stream.flush().await.is_err()
        {
            return Err(Stale);
        }
        read_response(stream).await
    }
}

struct Stale;

/// How an answer's body is delimited.
enum Framing {
    Chunked,
    Length(usize),
    ToEnd,
}

/// An answer's status line and headers, and the bytes read past them.
struct Head {
    status: u16,
    session: Option<String>,
    more: bool,
    close: bool,
    framing: Framing,
    rest: Vec<u8>,
}

enum BodyError {
    /// The body is longer than the limit it was read with.
    TooLarge,
    Failed(String),
}

impl From<std::io::Error> for BodyError {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}

impl BodyError {
    fn message(self) -> String {
        match self {
            Self::TooLarge => "the server's answer is too long".to_owned(),
            Self::Failed(error) => error,
        }
    }
}

const CLOSED_MID_ANSWER: &str = "the server closed the connection mid-answer";

async fn read_response(stream: &mut Box<dyn Io>) -> Result<Result<Response, String>, Stale> {
    let head = match read_head(stream).await? {
        Ok(head) => head,
        Err(error) => return Ok(Err(error)),
    };
    let close = head.close;
    let body = read_body(stream, head.framing, head.rest, usize::MAX).await;
    Ok(body.map_err(BodyError::message).map(|body| Response {
        status: head.status,
        session: head.session,
        more: head.more,
        close,
        body,
    }))
}

async fn read_head(stream: &mut Box<dyn Io>) -> Result<Result<Head, String>, Stale> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let size = loop {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut response = httparse::Response::new(&mut headers);
        match response.parse(&buffer) {
            Ok(httparse::Status::Complete(size)) => break size,
            Ok(httparse::Status::Partial) if buffer.len() <= MAX_HEAD_BYTES => {}
            _ => return Ok(Err("the server's answer is not HTTP".to_owned())),
        }
        let read = match stream.read(&mut chunk).await {
            Ok(read) => read,
            Err(_) if buffer.is_empty() => return Err(Stale),
            Err(error) => return Ok(Err(error.to_string())),
        };
        if read == 0 {
            return match buffer.is_empty() {
                true => Err(Stale),
                false => Ok(Err(CLOSED_MID_ANSWER.to_owned())),
            };
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut parsed = httparse::Response::new(&mut headers);
    let _ = parsed.parse(&buffer);
    let header = |name: &str| {
        parsed
            .headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .and_then(|header| std::str::from_utf8(header.value).ok())
            .map(|value| value.trim().to_owned())
    };
    let chunked = header("transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"));
    let length = match header("content-length") {
        Some(value) => match value.parse::<usize>() {
            Ok(length) => Some(length),
            Err(_) => return Ok(Err("the server's answer has a malformed length".to_owned())),
        },
        None => None,
    };
    let framing = match (chunked, length) {
        (true, _) => Framing::Chunked,
        (false, Some(length)) => Framing::Length(length),
        (false, None) => Framing::ToEnd,
    };
    let close = header("connection").is_some_and(|value| value.eq_ignore_ascii_case("close"))
        || matches!(framing, Framing::ToEnd);
    Ok(Ok(Head {
        status: parsed.code.unwrap_or_default(),
        session: header(SESSION_HEADER),
        more: header(MORE_HEADER).is_some(),
        close,
        framing,
        rest: buffer.split_off(size),
    }))
}

/// Reads a body delimited by `framing`, failing with `TooLarge` once it is longer than `limit`.
async fn read_body(
    stream: &mut Box<dyn Io>,
    framing: Framing,
    rest: Vec<u8>,
    limit: usize,
) -> Result<Vec<u8>, BodyError> {
    match framing {
        Framing::Chunked => dechunk(stream, rest, limit).await,
        Framing::Length(length) if length > limit => Err(BodyError::TooLarge),
        Framing::Length(length) => exactly(stream, rest, length).await,
        Framing::ToEnd => to_end(stream, rest, limit).await,
    }
}

async fn exactly(
    stream: &mut Box<dyn Io>,
    mut body: Vec<u8>,
    length: usize,
) -> Result<Vec<u8>, BodyError> {
    let mut chunk = [0u8; 8192];
    while body.len() < length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(BodyError::Failed(CLOSED_MID_ANSWER.to_owned()));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    Ok(body)
}

async fn to_end(
    stream: &mut Box<dyn Io>,
    mut body: Vec<u8>,
    limit: usize,
) -> Result<Vec<u8>, BodyError> {
    let mut chunk = [0u8; 8192];
    loop {
        if body.len() > limit {
            return Err(BodyError::TooLarge);
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(body);
        }
        body.extend_from_slice(&chunk[..read]);
    }
}

async fn dechunk(
    stream: &mut Box<dyn Io>,
    mut pending: Vec<u8>,
    limit: usize,
) -> Result<Vec<u8>, BodyError> {
    let mut body = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let Some(line_end) = pending.windows(2).position(|window| window == b"\r\n") else {
            if pending.len() > MAX_HEAD_BYTES {
                return Err(BodyError::Failed(
                    "the server's answer has a malformed chunk".to_owned(),
                ));
            }
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                return Err(BodyError::Failed(CLOSED_MID_ANSWER.to_owned()));
            }
            pending.extend_from_slice(&chunk[..read]);
            continue;
        };
        let line = String::from_utf8_lossy(&pending[..line_end]).to_string();
        let size = usize::from_str_radix(line.split(';').next().unwrap_or_default().trim(), 16)
            .map_err(|_| {
                BodyError::Failed("the server's answer has a malformed chunk".to_owned())
            })?;
        if size > limit.saturating_sub(body.len()) {
            return Err(BodyError::TooLarge);
        }
        while pending.len() < line_end + 2 + size + 2 {
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                return Err(BodyError::Failed(CLOSED_MID_ANSWER.to_owned()));
            }
            pending.extend_from_slice(&chunk[..read]);
        }
        if size == 0 {
            return Ok(body);
        }
        body.extend_from_slice(&pending[line_end + 2..line_end + 2 + size]);
        pending.drain(..line_end + 2 + size + 2);
    }
}

/// The longest refusal read to describe a failed request.
const REFUSAL_BYTES: usize = 16 * 1024;

/// Sends one `method` request to `url` with `headers` and `body`, on a connection of its own,
/// and answers the body of a 2xx answer, refusing one longer than `limit`. `Host`,
/// `Content-Length`, `Connection` and `Transfer-Encoding` are the request's own and are dropped
/// from `headers`. The gateway's authorization goes along when `url` is a server's that has one,
/// as on every other request to it.
pub(crate) async fn request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
    limit: usize,
) -> Result<Vec<u8>, crate::http::Error> {
    use crate::http::Error;
    let target = Target::parse(url).map_err(Error::Invalid)?;
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        target.path,
        target.host_header()
    );
    if method != "GET" || !body.is_empty() {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if let Some(authorization) = crate::authorization(url) {
        head.push_str(&format!("Be3-Gateway-Authorization: {authorization}\r\n"));
    }
    for (name, value) in headers {
        if ["host", "content-length", "connection", "transfer-encoding"]
            .iter()
            .any(|own| name.eq_ignore_ascii_case(own))
        {
            continue;
        }
        let token = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte));
        if !token
            || value
                .bytes()
                .any(|byte| byte == b'\r' || byte == b'\n' || byte == 0)
        {
            return Err(Error::Invalid(format!("the header {name:?} is not valid")));
        }
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let moved = Moved::new();
    let exchange = async {
        let mut stream: Box<dyn Io> = Box::new(Moving {
            inner: target.open().await?,
            moved: moved.clone(),
        });
        stream
            .write_all(head.as_bytes())
            .await
            .map_err(|error| error.to_string())?;
        stream
            .write_all(body)
            .await
            .map_err(|error| error.to_string())?;
        stream.flush().await.map_err(|error| error.to_string())?;
        let head = read_head(&mut stream)
            .await
            .unwrap_or_else(|Stale| Err("the server closed the connection".to_owned()))?;
        let status = head.status;
        if (200..300).contains(&status) {
            return Ok(
                match read_body(&mut stream, head.framing, head.rest, limit).await {
                    Ok(body) => Ok(body),
                    Err(BodyError::TooLarge) => Err(Error::TooLarge { limit }),
                    Err(BodyError::Failed(error)) => Err(Error::Failed(error)),
                },
            );
        }
        let message = match read_body(&mut stream, head.framing, head.rest, REFUSAL_BYTES).await {
            Ok(body) => String::from_utf8_lossy(&body).trim().to_owned(),
            Err(_) => String::new(),
        };
        Ok(Err(Error::from_status(status, message)))
    };
    while_moving(exchange, &moved)
        .await
        .unwrap_or_else(|error| Err(Error::Failed(error)))
}

#[cfg(test)]
mod tests;
