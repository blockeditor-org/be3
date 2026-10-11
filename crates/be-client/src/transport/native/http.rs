use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

use super::{Frame, TUNNEL_WAIT, proxied_tls, proxy_for, tunnel};

const SESSION_HEADER: &str = "be3-session";
const REQUEST_WAIT: Duration = Duration::from_secs(120);
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

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
    };
    client.stream = Some(client.target.open().await?);
    let (posts, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let (incoming, frames) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(body) = outgoing.recv().await {
            let answered = tokio::time::timeout(REQUEST_WAIT, client.post(&body))
                .await
                .unwrap_or_else(|_| Err("the server did not answer in time".to_owned()));
            match answered {
                Ok(answer) => {
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
}

struct Response {
    status: u16,
    session: Option<String>,
    close: bool,
    body: Vec<u8>,
}

impl Client {
    async fn post(&mut self, body: &[u8]) -> Result<Vec<Vec<u8>>, String> {
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
        frames(&response.body)
    }

    async fn exchange(&mut self, body: &[u8]) -> Result<Result<Response, String>, Stale> {
        let stream = match self.stream.take() {
            Some(stream) => stream,
            None => match self.target.open().await {
                Ok(stream) => stream,
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
            head.push_str(&format!("Authorization: {authorization}\r\n"));
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

async fn read_response(stream: &mut Box<dyn Io>) -> Result<Result<Response, String>, Stale> {
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
            Err(error) if buffer.is_empty() => {
                let _ = error;
                return Err(Stale);
            }
            Err(error) => return Ok(Err(error.to_string())),
        };
        if read == 0 {
            return match buffer.is_empty() {
                true => Err(Stale),
                false => Ok(Err("the server closed the connection mid-answer".to_owned())),
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
    let status = parsed.code.unwrap_or_default();
    let session = header(SESSION_HEADER);
    let close = header("connection").is_some_and(|value| value.eq_ignore_ascii_case("close"));
    let chunked = header("transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"));
    let length = header("content-length").and_then(|value| value.parse::<usize>().ok());
    let rest = buffer.split_off(size);
    let body = match (chunked, length) {
        (true, _) => dechunk(stream, rest).await,
        (false, Some(length)) => exactly(stream, rest, length).await,
        (false, None) => to_end(stream, rest).await,
    };
    Ok(body.map(|body| Response {
        status,
        session,
        close: close || (!chunked && length.is_none()),
        body,
    }))
}

async fn exactly(
    stream: &mut Box<dyn Io>,
    mut body: Vec<u8>,
    length: usize,
) -> Result<Vec<u8>, String> {
    let mut chunk = [0u8; 8192];
    while body.len() < length {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("the server closed the connection mid-answer".to_owned());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    Ok(body)
}

async fn to_end(stream: &mut Box<dyn Io>, mut body: Vec<u8>) -> Result<Vec<u8>, String> {
    stream
        .read_to_end(&mut body)
        .await
        .map_err(|error| error.to_string())?;
    Ok(body)
}

async fn dechunk(stream: &mut Box<dyn Io>, mut pending: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let Some(line_end) = pending.windows(2).position(|window| window == b"\r\n") else {
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                return Err("the server closed the connection mid-answer".to_owned());
            }
            pending.extend_from_slice(&chunk[..read]);
            continue;
        };
        let line = String::from_utf8_lossy(&pending[..line_end]).to_string();
        let size = usize::from_str_radix(line.split(';').next().unwrap_or_default().trim(), 16)
            .map_err(|_| "the server's answer has a malformed chunk".to_owned())?;
        while pending.len() < line_end + 2 + size + 2 {
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                return Err("the server closed the connection mid-answer".to_owned());
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

fn frames(body: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut frames = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        if rest.len() < 4 {
            return Err("the server's answer ends partway through a frame".to_owned());
        }
        let size = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let frame = rest
            .get(4..4 + size)
            .ok_or_else(|| "the server's answer ends partway through a frame".to_owned())?;
        frames.push(frame.to_vec());
        rest = &rest[4 + size..];
    }
    Ok(frames)
}
