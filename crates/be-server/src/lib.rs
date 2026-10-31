mod browser_auth;
mod client;
pub mod mail;
mod portal;
use std::{
    error::Error,
    fmt, io,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use be_ops::Incoming;
use be_protocol::{ErrorCode, MAX_FRAME_BYTES, embed, hello, read_hello};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{Notify, Semaphore, mpsc},
};
use tokio_tungstenite::{
    WebSocketStream, accept_hdr_async_with_config,
    tungstenite::{
        Message,
        handshake::server::{Callback, ErrorResponse, Request, Response},
        protocol::WebSocketConfig,
    },
};

pub mod backup;
pub mod blocks;
pub mod contacts;
mod http;
pub mod keys;
mod organization;
pub mod schema;
pub mod sessions;
pub mod store;
pub mod watch;

pub use blocks::PublishOutcome;
pub use sessions::SessionRegistry;
pub use store::{Identity, ServerStore};
pub use watch::{Push, WatchHub};

use client::Client;

#[derive(Debug)]
pub enum ServerError {
    Refused(ErrorCode, String),
    Corrupt,
    Io(io::Error),
    Database(rusqlite::Error),
    Store(be_store::StoreError),
    Schema(sqlite_schema::SchemaError),
    Socket(String),
}

impl ServerError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Refused(code, _) => *code,
            Self::Corrupt | Self::Database(_) | Self::Store(_) | Self::Schema(_) | Self::Io(_) => {
                ErrorCode::Storage
            }
            Self::Socket(_) => ErrorCode::InvalidRequest,
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(_, message) => formatter.write_str(message),
            Self::Corrupt => formatter.write_str("stored server state is inconsistent"),
            Self::Io(error) => write!(formatter, "server io failed: {error}"),
            Self::Database(error) => write!(formatter, "server database failed: {error}"),
            Self::Store(error) => write!(formatter, "object store failed: {error}"),
            Self::Schema(error) => write!(formatter, "server database failed: {error}"),
            Self::Socket(message) => write!(formatter, "websocket failed: {message}"),
        }
    }
}

impl Error for ServerError {}

impl From<io::Error> for ServerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for ServerError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

impl From<sqlite_schema::SchemaError> for ServerError {
    fn from(error: sqlite_schema::SchemaError) -> Self {
        Self::Schema(error)
    }
}

impl From<be_store::StoreError> for ServerError {
    fn from(error: be_store::StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ServerError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Socket(error.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub allow_registration: bool,
    pub allow_login: bool,
    pub local_profiles: bool,
    pub auth_origin: Option<String>,
    pub lease: Duration,
    pub portal: Option<be_portal::Portal>,
}

impl ServerConfig {
    const CLOSED: Self = Self {
        allow_registration: false,
        allow_login: false,
        local_profiles: false,
        auth_origin: None,
        lease: Duration::from_millis(be_session::LEASE_MILLISECONDS.unsigned_abs()),
        portal: None,
    };

    pub const OPEN: Self = Self {
        allow_registration: true,
        allow_login: true,
        ..Self::CLOSED
    };
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::CLOSED
    }
}

const MAX_CONNECTIONS: usize = 1024;
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(30);
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
const OUTBOUND_FRAMES: usize = 64;
const INBOUND_FRAMES: usize = 16;
const PING_FRAME_BYTES: usize = 32;

fn is_ping(frame: &[u8]) -> bool {
    frame.len() <= PING_FRAME_BYTES
        && matches!(
            be_ops::decode_frame::<be_ops::Frame>(frame),
            Ok(be_ops::Frame::Start { method, .. }) if method == be_protocol::__args::CONNECTION_PING
        )
}

enum Arrived {
    Frame(Vec<u8>),
    Events(Vec<Incoming>),
}

fn pong(peer: &be_ops::net::Peer, frame: &[u8]) -> Vec<Incoming> {
    let Ok(events) = peer.receive(frame) else {
        return Vec::new();
    };
    let mut others = Vec::new();
    for event in events {
        match event {
            Incoming::Started(started) if embed::connection::Call::accepts(&started) => {
                let call = embed::connection::Call::accept(&mut peer.lock(), started);
                if let Some(embed::connection::Call::Ping { reply }) = call {
                    reply.send(&mut peer.lock(), &());
                }
            }
            other => others.push(other),
        }
    }
    others
}

const ACCEPT_RETRY: Duration = Duration::from_millis(100);
const CONTENTS_BYTES: usize = MAX_FRAME_BYTES / 2;

pub async fn serve_with_config(
    listener: TcpListener,
    data_dir: impl Into<PathBuf>,
    config: ServerConfig,
    shutdown: impl Future<Output = impl Sized>,
) -> Result<(), ServerError> {
    serve_with_mail(
        listener,
        data_dir,
        config,
        Arc::new(mail::LogDelivery),
        shutdown,
    )
    .await
}

pub async fn serve_with_mail(
    listener: TcpListener,
    data_dir: impl Into<PathBuf>,
    config: ServerConfig,
    mail: Arc<dyn mail::Delivery>,
    shutdown: impl Future<Output = impl Sized>,
) -> Result<(), ServerError> {
    if let Some(origin) = &config.auth_origin {
        mail::validate_origin(origin).map_err(ServerError::Socket)?;
    }
    let services = Services {
        portal: config
            .portal
            .map(|portal| Arc::new(portal::Served::new(portal))),
        store: Arc::new(ServerStore::open(data_dir.into())?),
        hub: Arc::new(WatchHub::new()),
        registry: Arc::new(SessionRegistry::new(config.lease)),
        config,
        mail,
        http: Arc::new(http::Sessions::default()),
        browser_auth: Arc::new(browser_auth::Sessions::default()),
    };
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let mut session_changes = services.store.session_changes.subscribe();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                services.http.close_all().await;
                services.store.checkpoint().await?;
                return Ok(());
            }
            _ = session_changes.changed() => { services.http.revoke_invalid(&services.store).await; }
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::warn!(%error, "accepting a connection failed");
                        tokio::time::sleep(ACCEPT_RETRY).await;
                        continue;
                    }
                };
                let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
                    tracing::warn!(%peer, "refusing a connection: the server is full");
                    continue;
                };
                let _ = stream.set_nodelay(true);
                let services = services.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(stream, peer, services).await {
                        tracing::debug!(%peer, %error, "a connection ended with an error");
                    }
                    drop(slot);
                });
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Services {
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
    config: ServerConfig,
    mail: Arc<dyn mail::Delivery>,
    http: Arc<http::Sessions>,
    browser_auth: Arc<browser_auth::Sessions>,
    portal: Option<Arc<portal::Served>>,
}

#[derive(Default)]
struct Handshake {
    forwarded: Option<String>,
    authorization: Option<String>,
}

struct Forwarded(Arc<std::sync::Mutex<Handshake>>);

impl Callback for Forwarded {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        self.0.lock().unwrap().forwarded = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(|value| value.trim().to_owned());
        self.0.lock().unwrap().authorization = request
            .headers()
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        Ok(response)
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    services: Services,
) -> Result<(), ServerError> {
    let Some(stream) = http::route(stream, &services).await? else {
        return Ok(());
    };
    let held = Arc::new(std::sync::Mutex::new(Handshake::default()));
    let handshake = accept_hdr_async_with_config(
        stream,
        Forwarded(Arc::clone(&held)),
        Some(WebSocketConfig {
            max_frame_size: Some(MAX_FRAME_BYTES),
            max_message_size: Some(MAX_FRAME_BYTES),
            ..WebSocketConfig::default()
        }),
    );
    let socket = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake)
        .await
        .map_err(|_| ServerError::Socket("the websocket handshake timed out".into()))??;
    let Handshake {
        forwarded,
        authorization,
    } = std::mem::take(&mut *held.lock().unwrap());
    let client = forwarded.unwrap_or_else(|| peer.ip().to_string());
    tracing::debug!(%client, "connected");
    serve_socket(socket, &services, authorization).await
}

pub(crate) fn greeting(theirs: Option<&str>) -> Result<(Vec<u8>, bool), ServerError> {
    let ours = build_id::current().map_err(ServerError::Socket)?;
    let compatible = theirs.is_some_and(|theirs| build_id::compatible(ours, theirs));
    if !compatible {
        tracing::info!(
            ours,
            theirs = theirs.unwrap_or("a build from before the greeting"),
            "refused a client of another build"
        );
    }
    Ok((hello(ours), compatible))
}

async fn greet<S>(
    sink: &mut SplitSink<WebSocketStream<S>, Message>,
    source: &mut SplitStream<WebSocketStream<S>>,
) -> Result<bool, ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let greeting_frame = async {
        loop {
            match source.next().await {
                Some(Ok(Message::Binary(bytes))) => return Ok(read_hello(&bytes)),
                Some(Ok(Message::Ping(payload))) => sink.send(Message::Pong(payload)).await?,
                Some(Ok(Message::Text(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Ok(Message::Close(_))) | None => return Ok(None),
                Some(Err(error)) => return Err(ServerError::from(error)),
            }
        }
    };
    let theirs = tokio::time::timeout(HANDSHAKE_TIMEOUT, greeting_frame)
        .await
        .map_err(|_| ServerError::Socket("the client never said which build it is".into()))??;
    let (ours, compatible) = greeting(theirs.as_deref())?;
    sink.send(Message::Binary(ours)).await?;
    if !compatible {
        sink.send(Message::Close(None)).await?;
    }
    Ok(compatible)
}

async fn serve_socket<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
    services: &Services,
    authorization: Option<String>,
) -> Result<(), ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut sink, mut source) = socket.split();
    if !greet(&mut sink, &mut source).await? {
        return Ok(());
    }
    let mut pings =
        tokio::time::interval_at(tokio::time::Instant::now() + PING_INTERVAL, PING_INTERVAL);
    let (outgoing, mut sending) = mpsc::unbounded_channel::<Message>();
    let queued = Arc::new(AtomicUsize::new(0));
    let room = Arc::new(Notify::new());
    let mut client = {
        let (outgoing, queued) = (outgoing.clone(), Arc::clone(&queued));
        Client::open(services, move |frame| {
            queued.fetch_add(1, Ordering::Relaxed);
            let _ = outgoing.send(Message::Binary(frame));
        })
        .await
    };
    let mut session_changes = services.store.session_changes.subscribe();
    client.connection.header_token = bearer(authorization.as_deref());
    let (arrived, mut arriving) = mpsc::channel::<Arrived>(INBOUND_FRAMES);
    let (finish, mut finished) = tokio::sync::oneshot::channel::<()>();

    let writing = {
        let (queued, room) = (Arc::clone(&queued), Arc::clone(&room));
        async move {
            let mut write = async |message: Message| {
                let binary = matches!(message, Message::Binary(_));
                sink.send(message).await?;
                if binary {
                    queued.fetch_sub(1, Ordering::Relaxed);
                    room.notify_one();
                }
                Ok::<_, ServerError>(())
            };
            loop {
                tokio::select! {
                    biased;
                    message = sending.recv() => match message {
                        Some(message) => write(message).await?,
                        None => return Ok(()),
                    },
                    _ = &mut finished => {
                        while let Ok(message) = sending.try_recv() {
                            write(message).await?;
                        }
                        return Ok::<_, ServerError>(());
                    }
                }
            }
        }
    };
    let reading = {
        let (outgoing, peer) = (outgoing.clone(), client.peer.clone());
        async move {
            loop {
                let Ok(frame) = tokio::time::timeout(IDLE_TIMEOUT, source.next()).await else {
                    return Ok(());
                };
                let Some(frame) = frame else { return Ok(()) };
                let sent = match frame? {
                    Message::Binary(bytes) if is_ping(&bytes) => {
                        let others = pong(&peer, &bytes);
                        others.is_empty() || arrived.send(Arrived::Events(others)).await.is_ok()
                    }
                    Message::Binary(bytes) => arrived.send(Arrived::Frame(bytes)).await.is_ok(),
                    Message::Ping(payload) => outgoing.send(Message::Pong(payload)).is_ok(),
                    Message::Close(_) => return Ok(()),
                    Message::Text(_) | Message::Pong(_) | Message::Frame(_) => true,
                };
                if !sent {
                    return Ok::<_, ServerError>(());
                }
            }
        }
    };
    let answering = {
        let client = &mut client;
        let (queued, room) = (Arc::clone(&queued), Arc::clone(&room));
        async move {
            loop {
                let roomy = queued.load(Ordering::Relaxed) < OUTBOUND_FRAMES;
                tokio::select! {
                    _ = session_changes.changed() => {
                        if let Some(token) = &client.connection.token
                            && services.store.resolve_token(token).await.is_err() {
                            let _ = outgoing.send(Message::Close(None));
                            return Ok(());
                        }
                    }
                    push = client.pushes.recv() => {
                        if let Some(token) = &client.connection.token
                            && services.store.resolve_token(token).await.is_err() {
                            let _ = outgoing.send(Message::Close(None));
                            return Ok(());
                        }
                        let Some(push) = push else { return Ok(()) };
                        if let Err(error) = client.push(push).await {
                            tracing::warn!(%error, "dropping a connection that is not reading what it is sent");
                            return Err(error);
                        }
                    }
                    late = client.lates.recv() => {
                        if let Some(late) = late {
                            client.late(late).await;
                        }
                    }
                    _ = pings.tick() => {
                        if outgoing.send(Message::Ping(Vec::new())).is_err() {
                            return Ok(());
                        }
                    }
                    arrived = arriving.recv(), if roomy => match arrived {
                        None => return Ok(()),
                        Some(Arrived::Frame(bytes)) => client.receive(&bytes).await?,
                        Some(Arrived::Events(events)) => client.handle(events).await,
                    },
                    () = room.notified(), if !roomy => {}
                }
            }
        }
    };
    let served = async {
        tokio::pin!(answering);
        let outcome = tokio::select! {
            outcome = &mut answering => outcome,
            outcome = reading => {
                match tokio::time::timeout(DRAIN_TIMEOUT, answering).await {
                    Ok(answered) => outcome.and(answered),
                    Err(_) => outcome,
                }
            }
        };
        let _ = finish.send(());
        outcome
    };
    let (outcome, written) = tokio::join!(served, writing);
    let outcome = outcome.and(written);

    client.close().await;
    outcome
}

#[cfg(test)]
mod tests;

fn bearer(header: Option<&str>) -> Option<String> {
    let (scheme, token) = header?.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("Bearer")
        && !token.is_empty()
        && !token.chars().any(char::is_whitespace))
    .then(|| token.to_owned())
}
