mod address;
mod browser_auth;
mod client;
pub mod mail;
mod portal;
mod running;
use std::{
    borrow::Cow,
    error::Error,
    fmt, io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use be_block_protocol::{ErrorCode, MAX_FRAME_BYTES, embed, hello, read_hello};
use be_wasm_ops::Incoming;
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
    WebSocketStream, accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

pub mod backup;
pub mod blocks;
pub mod contacts;
mod http;
pub mod keys;
pub mod limits;
mod organization;
pub mod s3;
pub mod schema;
pub mod sessions;
pub mod store;
pub mod watch;

pub use blocks::PublishOutcome;
pub use running::Running;
pub use sessions::SessionRegistry;
pub use store::{Identity, ServerStore};
pub use watch::{Place, Push, WatchHub};

use client::Client;

#[derive(Debug)]
pub enum ServerError {
    Refused(ErrorCode, String),
    Corrupt,
    Io(io::Error),
    Database(rusqlite::Error),
    Store(be_block_store::StoreError),
    Schema(be_block_sqlite_schema::SchemaError),
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

impl From<be_block_sqlite_schema::SchemaError> for ServerError {
    fn from(error: be_block_sqlite_schema::SchemaError) -> Self {
        Self::Schema(error)
    }
}

impl From<be_block_store::StoreError> for ServerError {
    fn from(error: be_block_store::StoreError) -> Self {
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
    pub portal: Option<be_server_portal::Portal>,
    /// The bytes each account may store (`--account-quota`): the distinct objects of the blocks
    /// it pays for, and its uploads no block holds yet. None for no limit.
    pub account_quota: Option<u64>,
    /// How long an upload no block holds is kept before it is collected.
    pub upload_grace: Duration,
    /// Peers whose `X-Forwarded-For` names the client; anyone else's is ignored. By default
    /// only this host, where the reverse proxy runs.
    pub trusted_proxies: Cow<'static, [IpAddr]>,
    /// How long a connection may go after its greeting before its first call signs it in.
    pub sign_in_timeout: Duration,
    /// The fewest seconds between two polls of a browser sign-in, at first; tests that poll
    /// one sign-in again at once set it to zero.
    pub sign_in_poll_interval: Duration,
}

impl ServerConfig {
    const CLOSED: Self = Self {
        allow_registration: false,
        allow_login: false,
        local_profiles: false,
        auth_origin: None,
        lease: Duration::from_millis(be_block_session::LEASE_MILLISECONDS.unsigned_abs()),
        portal: None,
        account_quota: Some(store::DEFAULT_ACCOUNT_QUOTA),
        upload_grace: store::DEFAULT_UPLOAD_GRACE,
        trusted_proxies: Cow::Borrowed(&[
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ]),
        sign_in_timeout: SIGN_IN_TIMEOUT,
        sign_in_poll_interval: browser_auth::POLL_INTERVAL,
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
/// Connections one client address (see [`address::address_key`]) may hold open at once.
const MAX_CONNECTIONS_PER_ADDRESS: usize = 64;
/// Connections not signed in yet, counted from their first byte until they sign in. Taking one
/// past this lets go of the oldest (`address::Unsigned`), so they never hold more than this of
/// `MAX_CONNECTIONS`, and signed-in clients always have the rest.
const MAX_UNSIGNED_CONNECTIONS: usize = MAX_CONNECTIONS / 4;
/// Connections not signed in that one address (see [`address::unsigned_key`]: an IPv6 /48)
/// may hold at once.
pub(crate) const MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS: usize = 8;
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// The default for [`ServerConfig::sign_in_timeout`].
const SIGN_IN_TIMEOUT: Duration = HANDSHAKE_TIMEOUT;
const PING_INTERVAL: Duration = Duration::from_secs(30);
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
/// How often idle, unauthenticated and overflowing HTTP sessions are swept away.
const SWEEP_INTERVAL: Duration = Duration::from_secs(10);
/// While more than this many frames, or `OUTBOUND_BYTES`, wait to be written, a connection's loop
/// takes no new frame, so a client that does not read its socket is slowed rather than buffered.
const OUTBOUND_FRAMES: usize = 64;
pub(crate) const OUTBOUND_BYTES: usize = MAX_FRAME_BYTES;
/// What one connection may have waiting to be written, from replies and every stream together,
/// before it is closed: a client that does not read what it is sent is disconnected rather
/// than buffered.
pub(crate) const MAX_OUTBOUND_BYTES: usize = 8 * MAX_FRAME_BYTES;
const INBOUND_FRAMES: usize = 16;
const PING_FRAME_BYTES: usize = 32;

/// What a connection has waiting to be written.
#[derive(Default)]
pub(crate) struct Outbound {
    frames: AtomicUsize,
    bytes: AtomicUsize,
    overflowed: AtomicBool,
    room: Notify,
}

impl Outbound {
    /// Counts a frame about to be queued, or refuses it and marks the connection overflowed when
    /// it would hold more than `MAX_OUTBOUND_BYTES`.
    pub(crate) fn queue(&self, size: usize) -> bool {
        if self.overflowed.load(Ordering::Relaxed) {
            return false;
        }
        if self.bytes.fetch_add(size, Ordering::Relaxed) + size > MAX_OUTBOUND_BYTES {
            self.bytes.fetch_sub(size, Ordering::Relaxed);
            self.overflowed.store(true, Ordering::Relaxed);
            self.room.notify_one();
            return false;
        }
        self.frames.fetch_add(1, Ordering::Relaxed);
        true
    }

    pub(crate) fn sent(&self, size: usize) {
        self.frames.fetch_sub(1, Ordering::Relaxed);
        self.bytes.fetch_sub(size, Ordering::Relaxed);
        self.room.notify_one();
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes.load(Ordering::Relaxed)
    }

    fn roomy(&self) -> bool {
        self.frames.load(Ordering::Relaxed) < OUTBOUND_FRAMES && self.bytes() < OUTBOUND_BYTES
    }

    pub(crate) fn overflowed(&self) -> bool {
        self.overflowed.load(Ordering::Relaxed)
    }
}

fn not_reading() -> ServerError {
    ServerError::Socket("the client is not reading what it is sent".into())
}

fn is_ping(frame: &[u8]) -> bool {
    frame.len() <= PING_FRAME_BYTES
        && matches!(
            be_wasm_ops::decode_frame::<be_wasm_ops::Frame>(frame),
            Ok(be_wasm_ops::Frame::Start { method, .. }) if method == be_block_protocol::__args::CONNECTION_PING
        )
}

enum Arrived {
    Frame(Vec<u8>),
    Events(Vec<Incoming>),
}

fn pong(peer: &be_wasm_ops::net::Peer, frame: &[u8]) -> Vec<Incoming> {
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
    let interval = config.sign_in_poll_interval;
    let store = Arc::new(ServerStore::open_with(
        data_dir.into(),
        store::StorageLimits {
            account_quota: config.account_quota,
            upload_grace: config.upload_grace,
        },
    )?);
    let services = Services {
        portal: config
            .portal
            .map(|portal| Arc::new(portal::Served::new(portal))),
        s3: s3::service(&store.s3_endpoint(), Arc::new(store.objects().clone())),
        store,
        hub: Arc::new(WatchHub::new()),
        registry: Arc::new(SessionRegistry::new(config.lease)),
        config,
        mail,
        http: Arc::new(http::Sessions::default()),
        browser_auth: Arc::new(browser_auth::Sessions::polled_every(interval)),
        addresses: address::PerAddress::new(MAX_CONNECTIONS_PER_ADDRESS),
        unsigned: address::Unsigned::new(
            MAX_UNSIGNED_CONNECTIONS,
            MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS,
        ),
        calls: Arc::new(address::Rate::new(
            http::MAX_CALLS_PER_ADDRESS,
            http::CALLS_WINDOW,
        )),
    };
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let upkeep = tokio::spawn(upkeep(services.clone()));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                upkeep.abort();
                services.http.close_all().await;
                services.store.checkpoint().await?;
                return Ok(());
            }
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
                // A proxy speaks for many clients, so its connections are counted by the
                // address it forwards, once the request head is read.
                let eviction = Arc::new(address::Eviction::default());
                let (place, anonymous) = match services.config.trusted_proxies.contains(&peer.ip().to_canonical()) {
                    true => (None, None),
                    false => match (
                        services.addresses.take(peer.ip()),
                        services.unsigned.admit(peer.ip(), &eviction),
                    ) {
                        (Some(place), Some(anonymous)) => (Some(place), Some(anonymous)),
                        _ => {
                            tracing::debug!(%peer, "refusing a connection: its address holds too many");
                            continue;
                        }
                    },
                };
                let _ = stream.set_nodelay(true);
                let services = services.clone();
                tokio::spawn(async move {
                    let admitted = Admitted { place, anonymous, eviction };
                    if let Err(error) = handle_connection(stream, peer, admitted, services).await {
                        tracing::debug!(%peer, %error, "a connection ended with an error");
                    }
                    drop(slot);
                });
            }
        }
    }
}

/// Sweeps HTTP sessions and abandoned uploads on timers and closes sessions whose account token
/// is revoked, away from the accept loop so accepting never waits on them.
async fn upkeep(services: Services) {
    let mut sweeps = tokio::time::interval(SWEEP_INTERVAL);
    let mut upload_sweeps = tokio::time::interval(store::usage::UPLOAD_SWEEP_INTERVAL);
    let mut session_changes = services.store.session_changes.subscribe();
    loop {
        tokio::select! {
            _ = sweeps.tick() => services.http.sweep().await,
            _ = upload_sweeps.tick() => {
                if let Err(error) = services.store.collect_uploads().await {
                    tracing::warn!(%error, "collecting abandoned uploads failed");
                }
            }
            changed = session_changes.changed() => {
                if changed.is_err() {
                    return;
                }
                services.http.revoke_invalid(&services.store).await;
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
    addresses: Arc<address::PerAddress>,
    /// Connections and requests that are not signed in.
    unsigned: Arc<address::Unsigned>,
    /// One-shot calls (`http::one_shot`) per address.
    calls: Arc<address::Rate>,
    /// The S3 endpoint over the server's objects (`s3`).
    s3: s3s::service::S3Service,
}

/// What a connection was let in with: its place among its address's connections and among
/// those not signed in, both taken later for a trusted proxy's, and what tells it it was let go
/// of to make room.
pub(crate) struct Admitted {
    place: Option<address::Held>,
    anonymous: Option<address::Anonymous>,
    eviction: Arc<address::Eviction>,
}

impl Services {
    /// The address a request comes from; see [`address::client_address`].
    pub(crate) fn client_address(&self, peer: IpAddr, forwarded: Option<&str>) -> IpAddr {
        address::client_address(peer, forwarded, &self.config.trusted_proxies)
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    admitted: Admitted,
    services: Services,
) -> Result<(), ServerError> {
    let eviction = Arc::clone(&admitted.eviction);
    let routed = tokio::select! {
        routed = http::route(stream, peer, admitted, &services) => routed?,
        () = eviction.wait() => return Ok(()),
    };
    let Some(http::Upgrade {
        stream,
        client,
        authorization,
        place: _place,
        anonymous,
    }) = routed
    else {
        return Ok(());
    };
    let (stream, heard) = Watched::new(stream);
    let handshake = accept_async_with_config(
        stream,
        Some(WebSocketConfig {
            max_frame_size: Some(MAX_FRAME_BYTES),
            max_message_size: Some(MAX_FRAME_BYTES),
            ..WebSocketConfig::default()
        }),
    );
    let socket = tokio::select! {
        socket = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake) => socket
            .map_err(|_| ServerError::Socket("the websocket handshake timed out".into()))??,
        () = eviction.wait() => return Ok(()),
    };
    tracing::debug!(%client, "connected");
    serve_socket(
        socket,
        &heard,
        &services,
        client,
        authorization,
        (anonymous, eviction),
    )
    .await
}

/// When a connection last read any bytes, so a message still arriving is not taken for silence.
#[derive(Clone)]
pub(crate) struct Heard(Arc<std::sync::Mutex<tokio::time::Instant>>);

impl Heard {
    fn last(&self) -> tokio::time::Instant {
        *self.0.lock().unwrap()
    }
}

/// A stream that notes in its `Heard` each time it reads.
pub(crate) struct Watched<S> {
    inner: S,
    heard: Heard,
}

impl<S> Watched<S> {
    pub(crate) fn new(inner: S) -> (Self, Heard) {
        let heard = Heard(Arc::new(std::sync::Mutex::new(tokio::time::Instant::now())));
        (
            Self {
                inner,
                heard: heard.clone(),
            },
            heard,
        )
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Watched<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let before = buffer.filled().len();
        let polled = std::pin::Pin::new(&mut self.inner).poll_read(context, buffer);
        if buffer.filled().len() > before {
            *self.heard.0.lock().unwrap() = tokio::time::Instant::now();
        }
        polled
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Watched<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

/// The next message, or none once nothing at all has been read for `IDLE_TIMEOUT`. A large
/// message on a slow link takes as long as it takes, as long as its bytes keep coming.
pub(crate) async fn next_heard<M>(
    source: &mut (impl futures_util::Stream<Item = M> + Unpin),
    heard: &Heard,
) -> Option<M> {
    let mut next = source.next();
    loop {
        tokio::select! {
            message = &mut next => return message,
            () = tokio::time::sleep_until(heard.last() + IDLE_TIMEOUT) => {
                if heard.last() + IDLE_TIMEOUT <= tokio::time::Instant::now() {
                    return None;
                }
            }
        }
    }
}

pub(crate) fn greeting(theirs: Option<&str>) -> Result<(Vec<u8>, bool), ServerError> {
    let ours = be_lib_build_id::current().map_err(ServerError::Socket)?;
    let compatible = theirs.is_some_and(|theirs| be_lib_build_id::compatible(ours, theirs));
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
    heard: &Heard,
    services: &Services,
    address: IpAddr,
    authorization: Option<String>,
    anonymous: (address::Anonymous, Arc<address::Eviction>),
) -> Result<(), ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut sink, mut source) = socket.split();
    let greeted = tokio::select! {
        greeted = greet(&mut sink, &mut source) => greeted?,
        () = anonymous.1.wait() => return Ok(()),
    };
    if !greeted {
        return Ok(());
    }
    let mut pings =
        tokio::time::interval_at(tokio::time::Instant::now() + PING_INTERVAL, PING_INTERVAL);
    let (outgoing, mut sending) = mpsc::unbounded_channel::<Message>();
    let queued = Arc::new(Outbound::default());
    let mut client = {
        let (outgoing, queued) = (outgoing.clone(), Arc::clone(&queued));
        Client::open(services, address, false, anonymous, move |frame| {
            if queued.queue(frame.len()) {
                let _ = outgoing.send(Message::Binary(frame));
            }
        })
        .await
    };
    let mut session_changes = services.store.session_changes.subscribe();
    client.connection.header_token = bearer(authorization.as_deref());
    let (arrived, mut arriving) = mpsc::channel::<Arrived>(INBOUND_FRAMES);
    let (finish, mut finished) = tokio::sync::oneshot::channel::<()>();

    let writing = {
        let queued = Arc::clone(&queued);
        async move {
            let mut write = async |message: Message| {
                let binary = match &message {
                    Message::Binary(frame) => Some(frame.len()),
                    _ => None,
                };
                sink.send(message).await?;
                if let Some(size) = binary {
                    queued.sent(size);
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
                let Some(frame) = next_heard(&mut source, heard).await else {
                    return Ok(());
                };
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
        let queued = Arc::clone(&queued);
        async move {
            loop {
                if queued.overflowed() {
                    tracing::warn!("dropping a connection that is not reading what it is sent");
                    return Err(not_reading());
                }
                let roomy = queued.roomy();
                let deadline = client.connection.sign_in_deadline();
                let eviction = Arc::clone(&client.connection.eviction);
                tokio::select! {
                    () = eviction.wait(), if deadline.is_some() => {
                        tracing::debug!("closing a connection let go of for a newer one that is not signed in");
                        let _ = outgoing.send(Message::Close(None));
                        return Ok(());
                    }
                    () = sleep_until_some(deadline) => {
                        if client.connection.sign_in_deadline().is_some_and(|deadline| deadline <= tokio::time::Instant::now()) {
                            tracing::debug!("closing a connection that did not sign in in time");
                            let _ = outgoing.send(Message::Close(None));
                            return Ok(());
                        }
                    }
                    _ = session_changes.changed() => {
                        if client.connection.signed_in().await.is_err() {
                            let _ = outgoing.send(Message::Close(None));
                            return Ok(());
                        }
                    }
                    push = client.pushes.recv() => {
                        if client.connection.signed_in().await.is_err() {
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
                    arrived = arriving.recv(), if roomy => {
                        match arrived {
                            None => return Ok(()),
                            Some(Arrived::Frame(bytes)) => client.receive(&bytes).await?,
                            Some(Arrived::Events(events)) => client.handle(events).await,
                        }
                        if client.connection.closing {
                            let _ = outgoing.send(Message::Close(None));
                            return Ok(());
                        }
                    }
                    () = queued.room.notified() => {}
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

async fn sleep_until_some(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
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
