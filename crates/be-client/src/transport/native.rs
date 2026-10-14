use std::future::Future;
use std::sync::Arc;

use be_protocol::MAX_FRAME_BYTES;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use rustls::pki_types::{CertificateDer, pem::PemObject};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, client_async_tls_with_config,
    connect_async_with_config,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};

use super::Frame;

mod http;

type Stream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(crate) enum Writer {
    Socket(SplitSink<Stream, Message>),
    Http(tokio::sync::mpsc::UnboundedSender<Vec<u8>>),
}

pub(crate) enum Reader {
    Socket(SplitStream<Stream>),
    Http(tokio::sync::mpsc::UnboundedReceiver<Frame>),
}

pub(crate) fn spawn(future: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(future);
}

pub(crate) async fn connect(url: &str) -> Result<(Writer, Reader), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let (writer, reader) = http::connect(url).await?;
        return Ok((Writer::Http(writer), Reader::Http(reader)));
    }
    let config = WebSocketConfig {
        max_frame_size: Some(MAX_FRAME_BYTES),
        max_message_size: Some(MAX_FRAME_BYTES),
        ..WebSocketConfig::default()
    };
    let mut request = url
        .into_client_request()
        .map_err(|error| error.to_string())?;
    if let Some(header) = crate::authorization(url) {
        request.headers_mut().insert(
            "authorization",
            header
                .parse()
                .map_err(|_| "the Authorization header is not valid".to_owned())?,
        );
    }
    let target = request.uri().clone();
    let host = target.host().unwrap_or_default().to_owned();
    let secure = target.scheme_str() == Some("wss");
    let socket = match proxy_for(&host).filter(|_| secure) {
        Some(proxy) => {
            let port = target.port_u16().unwrap_or(443);
            let stream = tokio::time::timeout(TUNNEL_WAIT, tunnel(&proxy, &host, port))
                .await
                .map_err(|_| format!("the proxy at {proxy} did not open a tunnel in time"))??;
            let (socket, _) = client_async_tls_with_config(
                request,
                stream,
                Some(config),
                Some(Connector::Rustls(Arc::new(proxied_tls()?))),
            )
            .await
            .map_err(|error| error.to_string())?;
            socket
        }
        None => {
            let (socket, _) = connect_async_with_config(request, Some(config), true)
                .await
                .map_err(|error| error.to_string())?;
            socket
        }
    };
    let (sink, source) = socket.split();
    Ok((Writer::Socket(sink), Reader::Socket(source)))
}

fn proxy_for(host: &str) -> Option<String> {
    let proxy = ["HTTPS_PROXY", "https_proxy"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|proxy| !proxy.trim().is_empty())?;
    let bypassed = ["NO_PROXY", "no_proxy"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .flat_map(|list| {
            list.split(',')
                .map(|entry| entry.trim().trim_start_matches('.').to_owned())
                .collect::<Vec<_>>()
        })
        .any(|entry| !entry.is_empty() && (host == entry || host.ends_with(&format!(".{entry}"))));
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]");
    (!bypassed && !local).then_some(proxy)
}

const TUNNEL_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

async fn tunnel(proxy: &str, host: &str, port: u16) -> Result<TcpStream, String> {
    let proxy = proxy.trim();
    let address = match proxy.split_once("://") {
        None => proxy,
        Some(("http", address)) => address,
        Some((scheme, _)) => {
            return Err(format!(
                "the proxy {proxy} is {scheme}, and only http:// proxies are supported"
            ));
        }
    };
    let address = address.trim_end_matches('/');
    let mut stream = TcpStream::connect(address)
        .await
        .map_err(|error| format!("could not reach the proxy at {address}: {error}"))?;
    stream
        .write_all(
            format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").as_bytes(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let mut answer = Vec::new();
    let mut byte = [0u8; 1];
    while !answer.ends_with(b"\r\n\r\n") {
        if answer.len() > 16 * 1024 {
            return Err("the proxy sent an overlong answer".to_owned());
        }
        let read = stream
            .read(&mut byte)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("the proxy closed the connection".to_owned());
        }
        answer.push(byte[0]);
    }
    let status = String::from_utf8_lossy(&answer);
    let line = status.lines().next().unwrap_or_default();
    match line.split_whitespace().nth(1) {
        Some("200") => Ok(stream),
        _ => Err(format!("the proxy refused {host}:{port}: {line}")),
    }
}

fn proxied_tls() -> Result<rustls::ClientConfig, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for name in ["SSL_CERT_FILE", "CURL_CA_BUNDLE", "REQUESTS_CA_BUNDLE"] {
        let Ok(path) = std::env::var(name) else {
            continue;
        };
        let certificates = CertificateDer::pem_file_iter(&path)
            .map_err(|error| format!("could not read {path}: {error}"))?;
        for certificate in certificates.flatten() {
            let _ = roots.add(certificate);
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| error.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth())
}

impl Writer {
    pub(crate) async fn send(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        self.send_message(Message::Binary(bytes)).await
    }

    pub(crate) async fn pong(&mut self, payload: Vec<u8>) -> Result<(), String> {
        self.send_message(Message::Pong(payload)).await
    }

    async fn send_message(&mut self, message: Message) -> Result<(), String> {
        match self {
            Self::Socket(sink) => sink.send(message).await.map_err(|error| error.to_string()),
            Self::Http(posts) => match message {
                Message::Binary(bytes) => posts
                    .send(bytes)
                    .map_err(|_| "the connection closed".to_owned()),
                _ => Ok(()),
            },
        }
    }
}

impl Reader {
    pub(crate) async fn next(&mut self) -> Option<Frame> {
        let source = match self {
            Self::Socket(source) => source,
            Self::Http(frames) => return frames.recv().await,
        };
        loop {
            return match source.next().await? {
                Ok(Message::Binary(bytes)) => Some(Frame::Binary(bytes)),
                Ok(Message::Ping(payload)) => Some(Frame::Ping(payload)),
                Ok(Message::Text(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                Ok(Message::Close(_)) | Err(_) => Some(Frame::Closed),
            };
        }
    }
}
