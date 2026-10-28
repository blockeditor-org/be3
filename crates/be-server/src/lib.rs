mod browser_auth;
pub mod mail;
mod portal;
use std::{
    collections::BTreeSet, error::Error, fmt, io, net::SocketAddr, path::PathBuf, sync::Arc,
    time::Duration,
};

use be_protocol::{
    BlockSummary, ByteBuf, ClientMessage, ErrorCode, Listing, MAX_FRAME_BYTES, ServerMessage,
    decode, encode, hello, read_hello,
};
use be_session::Claim;
use be_store::Hash;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc},
};
use tokio_tungstenite::{
    WebSocketStream, accept_hdr_async_with_config,
    tungstenite::{
        Message,
        handshake::server::{Callback, ErrorResponse, Request, Response},
        protocol::WebSocketConfig,
    },
};
use uuid::Uuid;

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
pub use watch::WatchHub;

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
const PING_FRAME_BYTES: usize = 64;

fn pong_for(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() > PING_FRAME_BYTES {
        return None;
    }
    match decode::<ClientMessage>(bytes) {
        Ok(ClientMessage::Ping { request }) => encode(&ServerMessage::Pong { request }).ok(),
        _ => None,
    }
}
const OUTBOUND_LIMIT: usize = 4096;
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

struct Connection {
    browser_auth: Arc<browser_auth::Sessions>,
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
    client: u64,
    account: Option<Uuid>,
    token: Option<String>,
    header_token: Option<String>,
    identity: Option<Identity>,
    config: ServerConfig,
}

pub(crate) struct Client {
    connection: Connection,
    outbound: mpsc::Receiver<ServerMessage>,
}

impl Client {
    pub(crate) async fn open(services: &Services) -> Self {
        let (sender, outbound) = mpsc::channel(OUTBOUND_LIMIT);
        let client = services.hub.register(sender).await;
        Self {
            connection: Connection {
                browser_auth: Arc::clone(&services.browser_auth),
                store: Arc::clone(&services.store),
                hub: Arc::clone(&services.hub),
                registry: Arc::clone(&services.registry),
                client,
                account: None,
                token: None,
                header_token: None,
                identity: None,
                config: services.config.clone(),
            },
            outbound,
        }
    }

    pub(crate) async fn answer(&mut self, bytes: &[u8]) -> Vec<ServerMessage> {
        let response = match decode::<ClientMessage>(bytes) {
            Err(_) => ServerMessage::Failed {
                request: 0,
                code: ErrorCode::InvalidRequest,
                message: "the frame is not a valid protocol message".into(),
            },
            Ok(request) => {
                let id = request.request();
                match self.connection.dispatch(request).await {
                    Ok(response) => response,
                    Err(error) => ServerMessage::Failed {
                        request: id,
                        code: error.code(),
                        message: error.to_string(),
                    },
                }
            }
        };
        let mut frames = self.pending();
        frames.push(response);
        frames
    }

    pub(crate) fn pending(&mut self) -> Vec<ServerMessage> {
        let mut frames = Vec::new();
        while let Ok(notification) = self.outbound.try_recv() {
            frames.push(notification);
        }
        frames
    }

    pub(crate) async fn close(self) {
        let Connection {
            hub,
            registry,
            client,
            ..
        } = self.connection;
        for (block, state) in registry.leave_all(client).await {
            let participants = registry.participants(block).await;
            hub.send_all(
                &participants,
                client,
                &ServerMessage::SessionChanged { block, state },
            )
            .await;
        }
        hub.remove(client).await;
    }
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
    let mut client = Client::open(services).await;
    let mut session_changes = services.store.session_changes.subscribe();
    client.connection.header_token = bearer(authorization.as_deref());
    let (outgoing, mut sending) = mpsc::channel::<Message>(OUTBOUND_FRAMES);
    let (arrived, mut arriving) = mpsc::channel::<Vec<u8>>(INBOUND_FRAMES);

    let writing = async move {
        while let Some(message) = sending.recv().await {
            sink.send(message).await?;
        }
        Ok::<_, ServerError>(())
    };
    let reading = {
        let outgoing = outgoing.clone();
        async move {
            loop {
                let Ok(frame) = tokio::time::timeout(IDLE_TIMEOUT, source.next()).await else {
                    return Ok(());
                };
                let Some(frame) = frame else { return Ok(()) };
                let sent = match frame? {
                    Message::Binary(bytes) => match pong_for(&bytes) {
                        Some(pong) => outgoing.send(Message::Binary(pong)).await.is_ok(),
                        None => arrived.send(bytes).await.is_ok(),
                    },
                    Message::Ping(payload) => outgoing.send(Message::Pong(payload)).await.is_ok(),
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
        async move {
            let send = |message: Message| outgoing.send(message);
            loop {
                tokio::select! {
                    _ = session_changes.changed() => {
                        if let Some(token) = &client.connection.token
                            && services.store.resolve_token(token).await.is_err() {
                            let _ = send(Message::Close(None)).await;
                            return Ok(());
                        }
                    }
                    notification = client.outbound.recv() => {
                        if let Some(token) = &client.connection.token
                            && services.store.resolve_token(token).await.is_err() {
                            let _ = send(Message::Close(None)).await;
                            return Ok(());
                        }
                        let Some(notification) = notification else { return Ok(()) };
                        let frame = encode(&notification).map_err(|_| ServerError::Corrupt)?;
                        if send(Message::Binary(frame)).await.is_err() {
                            arriving.close();
                        }
                    }
                    _ = pings.tick() => {
                        if send(Message::Ping(Vec::new())).await.is_err() {
                            arriving.close();
                        }
                    }
                    bytes = arriving.recv() => {
                        let Some(bytes) = bytes else { return Ok(()) };
                        for message in client.answer(&bytes).await {
                            let frame = encode(&message).map_err(|_| ServerError::Corrupt)?;
                            if send(Message::Binary(frame)).await.is_err() {
                                arriving.close();
                            }
                        }
                    }
                }
            }
        }
    };
    let served = async {
        tokio::pin!(answering);
        tokio::select! {
            outcome = &mut answering => outcome,
            outcome = reading => {
                match tokio::time::timeout(DRAIN_TIMEOUT, answering).await {
                    Ok(answered) => outcome.and(answered),
                    Err(_) => outcome,
                }
            }
        }
    };
    let (outcome, written) = tokio::join!(served, writing);
    let outcome = outcome.and(written);

    client.close().await;
    outcome
}

impl Connection {
    async fn authenticated(
        &mut self,
        request: u64,
        profile: store::Profile,
        token: String,
    ) -> ServerMessage {
        self.hub.join_account(self.client, profile.account).await;
        self.account = Some(profile.account);
        self.token = Some(token.clone());
        ServerMessage::Authenticated {
            request,
            account: profile.account,
            email: profile.email,
            display_name: profile.display_name,
            token,
        }
    }

    fn identity(&self) -> Result<Identity, ServerError> {
        self.identity.ok_or_else(|| {
            ServerError::Refused(
                ErrorCode::NotAuthenticated,
                "open an organization before using it".into(),
            )
        })
    }

    fn account(&self) -> Result<Uuid, ServerError> {
        self.account.ok_or_else(|| {
            ServerError::Refused(
                ErrorCode::NotAuthenticated,
                "authenticate before using this connection".into(),
            )
        })
    }

    async fn dispatch(&mut self, message: ClientMessage) -> Result<ServerMessage, ServerError> {
        if let Some(token) = &self.token {
            self.store.resolve_token(token).await?;
        }
        if self.identity.is_some() {
            self.identity = self.hub.identity(self.client).await;
        }
        match message {
            ClientMessage::BeginBrowserLogin { request } => {
                let (code, secret) = self
                    .browser_auth
                    .begin(self.client, Arc::clone(&self.hub))?;
                Ok(ServerMessage::BrowserLogin {
                    request,
                    code,
                    secret,
                })
            }
            ClientMessage::FinishBrowserLogin {
                request,
                code,
                secret,
            } => {
                if !self.config.allow_login {
                    return Err(ServerError::Refused(
                        ErrorCode::LoginDisabled,
                        "This server is not accepting sign-ins.".into(),
                    ));
                }
                let Some((account, issuer)) =
                    self.browser_auth.finish(self.client, &code, &secret)?
                else {
                    return Ok(ServerMessage::Ok { request });
                };
                let token = self.store.claim_browser_session(account, &issuer).await?;
                let profile = self.store.resolve_token(&token).await?;
                Ok(self.authenticated(request, profile, token).await)
            }
            ClientMessage::CreateLocalProfile {
                request,
                display_name,
            } => {
                if !self.config.local_profiles {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "local profiles are unavailable on this server".into(),
                    ));
                }
                let (profile, token) = self.store.create_local_profile(&display_name).await?;
                Ok(self.authenticated(request, profile, token).await)
            }
            ClientMessage::Ping { request } => Ok(ServerMessage::Pong { request }),
            ClientMessage::AuthenticateHeader { request } => {
                let token = self.header_token.clone().ok_or_else(|| {
                    ServerError::Refused(
                        ErrorCode::NotAuthenticated,
                        "no Bearer account token was supplied in Authorization".into(),
                    )
                })?;
                let profile = self.store.resolve_token(&token).await?;
                let mut response = self.authenticated(request, profile, token).await;
                if let ServerMessage::Authenticated { token, .. } = &mut response {
                    token.clear();
                }
                Ok(response)
            }
            ClientMessage::Authenticate { request, token } => {
                let profile = self.store.resolve_token(&token).await?;
                self.token = Some(token.clone());
                Ok(self.authenticated(request, profile, token).await)
            }
            ClientMessage::Logout { request } => {
                if let Some(token) = self.token.take() {
                    self.store.logout(&token).await?;
                }
                self.account = None;
                self.identity = None;
                self.hub.leave_account(self.client).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::Invite {
                request,
                invitation,
                organization,
                email,
                role,
                sealed,
                code,
                escrow,
            } => {
                self.store
                    .invite(
                        self.account()?,
                        invitation,
                        organization,
                        &email,
                        role,
                        sealed,
                        code,
                        escrow,
                    )
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::GetInvitation {
                request,
                invitation,
            } => Ok(ServerMessage::Invitation {
                request,
                invitation: self.store.invitation(self.account()?, invitation).await?,
            }),
            ClientMessage::AcceptInvitation {
                request,
                invitation,
                card,
                contact,
                escrow,
            } => {
                let account = self.account()?;
                let inviter = self
                    .store
                    .accept_invitation(account, invitation, card, contact, escrow)
                    .await?;
                self.contacts_changed(&[account, inviter]).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::CreateContactLink {
                request,
                link,
                card,
                code,
                grant,
            } => {
                self.store
                    .create_contact_link(self.account()?, link, card, code, grant)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::GetContactLink { request, link } => Ok(ServerMessage::ContactLink {
                request,
                link: self.store.contact_link(self.account()?, link).await?,
            }),
            ClientMessage::AcceptContactLink {
                request,
                link,
                card,
                contact,
                key,
            } => {
                let account = self.account()?;
                let accepted = self
                    .store
                    .accept_contact_link(account, link, card, contact, key)
                    .await?;
                if let Some(granted) = accepted.granted {
                    self.reannounce_in(
                        granted.organization,
                        vec![granted.block],
                        true,
                        granted.before,
                    )
                    .await;
                }
                self.contacts_changed(&[account, accepted.owner]).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListContactLinks { request } => Ok(ServerMessage::ContactLinks {
                request,
                links: self.store.contact_links(self.account()?).await?,
            }),
            ClientMessage::DeleteContactLink { request, link } => {
                self.store
                    .delete_contact_link(self.account()?, link)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::PutContact {
                request,
                contact,
                sealed,
            } => {
                let account = self.account()?;
                self.store.put_contact(account, contact, sealed).await?;
                self.contacts_changed(&[account]).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListContacts { request } => Ok(ServerMessage::Contacts {
                request,
                contacts: self.store.list_contacts(self.account()?).await?,
            }),
            ClientMessage::RemoveContact { request, account } => {
                let caller = self.account()?;
                let revoked = self.store.remove_contact(caller, account).await?;
                self.revoked(&revoked, &[caller, account]).await;
                self.contacts_changed(&[caller, account]).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListOrganizations { request } => Ok(ServerMessage::Organizations {
                request,
                organizations: self.store.list_organizations(self.account()?).await?,
            }),
            ClientMessage::CreateOrganization { request, name } => {
                let organization = self
                    .store
                    .create_organization(self.account()?, &name)
                    .await?;
                Ok(ServerMessage::Organizations {
                    request,
                    organizations: vec![organization],
                })
            }
            ClientMessage::OpenOrganization {
                request,
                organization,
            } => {
                let account = self.account()?;
                let role = self.store.membership(account, organization).await?;
                self.identity = Some(Identity {
                    account,
                    organization,
                    role,
                });
                self.hub
                    .join_organization(
                        self.client,
                        Identity {
                            account,
                            organization,
                            role,
                        },
                    )
                    .await;
                let (kind, escrows, pinned) = self.store.opened(account, organization).await?;
                Ok(ServerMessage::OrganizationOpened {
                    request,
                    organization,
                    role,
                    kind,
                    escrows,
                    pinned,
                    server: self.store.server_account(),
                })
            }
            ClientMessage::PutObjects { request, objects } => {
                let identity = self.identity()?;
                self.store
                    .may_store(
                        identity,
                        objects.iter().map(|bytes| Hash::of(bytes)).collect(),
                    )
                    .await?;
                Ok(ServerMessage::Stored {
                    request,
                    hashes: self.store.put_objects(&objects)?,
                })
            }
            ClientMessage::GetObject { request, hash } => {
                let identity = self.identity()?;
                let bytes = match self.store.may_read_object(identity, hash).await? {
                    true => self.store.get_object(hash)?,
                    false => None,
                };
                Ok(ServerMessage::Object { request, bytes })
            }
            ClientMessage::MissingObjects { request, hashes } => {
                self.identity()?;
                Ok(ServerMessage::Missing {
                    request,
                    hashes: self.store.missing_objects(&hashes)?,
                })
            }

            ClientMessage::CreateBlock {
                request,
                block,
                content_type,
                parent,
                metadata,
                references,
                key,
                keys,
            } => {
                let identity = self.identity()?;
                let block = self
                    .store
                    .create_block(
                        identity,
                        block,
                        content_type,
                        parent,
                        metadata,
                        references,
                        key,
                        keys,
                    )
                    .await?;
                let mut announced = vec![block.id];
                announced.extend(block.references.iter().copied());
                self.reannounce(identity, announced, false, BTreeSet::new())
                    .await;
                Ok(ServerMessage::Block { request, block })
            }
            ClientMessage::SetMetadata {
                request,
                block,
                metadata,
                references,
            } => {
                let identity = self.identity()?;
                let (block, relinked) = self
                    .store
                    .set_metadata(identity, block, metadata, references)
                    .await?;
                let mut announced = vec![block.id];
                announced.extend(relinked);
                self.reannounce(identity, announced, false, BTreeSet::new())
                    .await;
                Ok(ServerMessage::Block { request, block })
            }
            ClientMessage::Publish {
                request,
                block,
                commit,
                expected,
                parents,
                chunks,
                time,
                pinned,
                references,
                metadata,
                objects,
            } => {
                let identity = self.identity()?;
                self.store.put_objects(&objects)?;
                let outcome = self
                    .store
                    .publish(
                        identity, block, commit, expected, parents, chunks, time, pinned,
                        references, metadata,
                    )
                    .await?;
                if let PublishOutcome::Published(_, Some(unlinked)) = &outcome {
                    let mut blocks = vec![block];
                    blocks.extend(unlinked);
                    self.reannounce(identity, blocks, false, BTreeSet::new())
                        .await;
                }
                Ok(match outcome {
                    PublishOutcome::Published(head, _) => {
                        self.hub
                            .broadcast(
                                block,
                                self.client,
                                ServerMessage::HeadChanged {
                                    block,
                                    head,
                                    author: identity.account,
                                },
                            )
                            .await;
                        ServerMessage::Published { request, head }
                    }
                    PublishOutcome::Rejected(head) => ServerMessage::Rejected { request, head },
                })
            }
            ClientMessage::HoldObjects {
                request,
                block,
                objects,
            } => {
                self.store
                    .hold_objects(self.identity()?, block, objects)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ReadBlock { request, block } => Ok(ServerMessage::Block {
                request,
                block: self
                    .list(Listing::Block(block))
                    .await?
                    .pop()
                    .ok_or(ServerError::Corrupt)?,
            }),
            ClientMessage::ReadContents {
                request,
                blocks,
                held,
            } => {
                let (blocks, hashes) = self
                    .store
                    .read_contents(self.identity()?, blocks, held)
                    .await?;
                let mut objects = Vec::new();
                let mut room = CONTENTS_BYTES;
                for hash in hashes {
                    let Some(bytes) = self.store.get_object(hash)? else {
                        continue;
                    };
                    if bytes.len() > room {
                        break;
                    }
                    room -= bytes.len();
                    objects.push(ByteBuf::from(bytes));
                }
                Ok(ServerMessage::Contents {
                    request,
                    blocks,
                    objects,
                })
            }
            ClientMessage::SetParent {
                request,
                block,
                parent,
                keys,
            } => {
                let identity = self.identity()?;
                let (before, lost) = self.store.set_parent(identity, block, parent, keys).await?;
                self.reannounce(identity, vec![block], true, before.clone())
                    .await;
                if !lost.is_empty() {
                    self.key_work_changed(identity.organization, before).await;
                    for account in lost {
                        self.evict_sessions(identity.organization, account).await;
                    }
                }
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListShared { request } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.list(Listing::Shared).await?,
            }),
            ClientMessage::ListChildren { request, parent } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.list(Listing::Children(parent)).await?,
            }),
            ClientMessage::ListDeleted { request } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.list(Listing::Deleted).await?,
            }),
            ClientMessage::ListBackrefs { request, block } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.list(Listing::Backrefs(block)).await?,
            }),
            ClientMessage::ListReferences { request, block } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.list(Listing::References(block)).await?,
            }),
            ClientMessage::ListOpenSettings { request, block } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.list(Listing::OpenSettings(block)).await?,
            }),
            ClientMessage::Unlist { request, listing } => {
                self.identity()?;
                self.hub.unlist(self.client, listing).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListHistory { request, block } => Ok(ServerMessage::History {
                request,
                entries: self.store.list_history(self.identity()?, block).await?,
            }),
            ClientMessage::PruneHistory {
                request,
                block,
                drop,
            } => {
                let objects = self
                    .store
                    .prune_history(self.identity()?, block, drop)
                    .await?;
                Ok(ServerMessage::Collected {
                    request,
                    blocks: Vec::new(),
                    objects,
                })
            }
            ClientMessage::DeleteBlock { request, block } => {
                let identity = self.identity()?;
                let before = self.store.delete_block(identity, block).await?;
                self.reannounce(identity, vec![block], true, before).await;
                self.hub
                    .broadcast(block, self.client, ServerMessage::BlockDeleted { block })
                    .await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::CollectDetached { request } => {
                let identity = self.identity()?;
                let collected = self.store.collect_detached(identity).await?;
                for block in &collected.blocks {
                    self.hub
                        .removed(identity.organization, *block, collected.revision)
                        .await;
                }
                Ok(ServerMessage::Collected {
                    request,
                    blocks: collected.blocks,
                    objects: collected.objects,
                })
            }
            ClientMessage::SetAccess {
                request,
                block,
                account,
                access,
            } => {
                let identity = self.identity()?;
                let before = self
                    .store
                    .set_access(identity, block, account, access)
                    .await?;
                self.revoked(
                    &[contacts::Granted {
                        organization: identity.organization,
                        block,
                        before,
                    }],
                    &[account],
                )
                .await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListAccess { request, block } => Ok(ServerMessage::AccessList {
                request,
                list: self.store.list_access(self.identity()?, block).await?,
            }),
            ClientMessage::Watch { request, block } => {
                self.identity()?;
                self.hub.watch(block, self.client).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::Unwatch { request, block } => {
                self.identity()?;
                self.hub.unwatch(block, self.client).await;
                Ok(ServerMessage::Ok { request })
            }

            ClientMessage::JoinSession { request, block } => {
                let identity = self.identity()?;
                self.store.read_block(identity, block).await?;
                let state = self.registry.join(block, self.client).await;
                self.announce(block, &state).await;
                Ok(ServerMessage::Session {
                    request,
                    block,
                    client: self.client,
                    state,
                })
            }
            ClientMessage::LeaveSession { request, block } => {
                self.identity()?;
                if let Some(state) = self.registry.leave(block, self.client).await {
                    self.announce(block, &state).await;
                }
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ClaimOwnership {
                request,
                block,
                generation,
            } => {
                self.identity()?;
                let (claim, state) = self.registry.claim(block, self.client, generation).await;
                if matches!(claim, Claim::Granted { .. }) {
                    self.announce(block, &state).await;
                }
                Ok(ServerMessage::Session {
                    request,
                    block,
                    client: self.client,
                    state,
                })
            }
            ClientMessage::Heartbeat {
                request,
                block,
                generation,
                clean_at,
            } => {
                self.identity()?;
                match self
                    .registry
                    .heartbeat(block, self.client, generation, clean_at)
                    .await
                {
                    Some(state) => Ok(ServerMessage::Session {
                        request,
                        block,
                        client: self.client,
                        state,
                    }),
                    None => Ok(ServerMessage::Session {
                        request,
                        block,
                        client: self.client,
                        state: self.registry.state(block).await,
                    }),
                }
            }
            ClientMessage::Relay {
                request,
                block,
                to,
                payload,
            } => {
                self.identity()?;
                let participants = self.registry.participants(block).await;
                if !participants.contains(&self.client) {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        "join the session before speaking in it".into(),
                    ));
                }
                let message = ServerMessage::Relayed {
                    block,
                    from: self.client,
                    payload,
                };
                match to {
                    Some(target) if participants.contains(&target) => {
                        self.hub.send_to(target, message).await;
                    }
                    Some(_) => {
                        return Err(ServerError::Refused(
                            ErrorCode::InvalidRequest,
                            "that client is not in this session".into(),
                        ));
                    }
                    None => {
                        self.hub
                            .send_all(&participants, self.client, &message)
                            .await;
                    }
                }
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::GetKeys { request, blocks } => Ok(ServerMessage::Keys {
                request,
                keys: self.store.get_keys(self.identity()?, blocks).await?,
            }),
            ClientMessage::PutKeys { request, keys } => {
                self.store.put_keys(self.identity()?, keys).await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::GetRecoveryKey { request } => {
                let (public, sealed, own) = self.store.recovery_key(self.account()?).await?;
                Ok(ServerMessage::RecoveryKey {
                    request,
                    public,
                    sealed,
                    own,
                })
            }
            ClientMessage::SetRecoveryKey {
                request,
                public,
                sealed,
                own,
                changes,
            } => {
                let account = self.account()?;
                let told: Vec<Uuid> = changes.iter().map(|change| change.contact).collect();
                self.store
                    .set_recovery_key(account, public, sealed, own, changes)
                    .await?;
                self.contacts_changed(&told).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ResealKeys { request, keys } => {
                self.store.reseal_keys(self.account()?, keys).await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::CreateAgent {
                request,
                organization,
                name,
                recovery,
                contact,
                escrow,
            } => {
                let account = self.account()?;
                let (agent, token) = self
                    .store
                    .create_agent(account, organization, &name, recovery, contact, escrow)
                    .await?;
                self.contacts_changed(&[account]).await;
                Ok(ServerMessage::AgentCreated {
                    request,
                    agent,
                    token,
                })
            }
            ClientMessage::ListAgents {
                request,
                organization,
            } => Ok(ServerMessage::Agents {
                request,
                agents: self
                    .store
                    .list_agents(self.account()?, organization)
                    .await?,
            }),
            ClientMessage::RemoveAgent {
                request,
                organization,
                account,
            } => {
                let revoked = self
                    .store
                    .remove_agent(self.account()?, organization, account)
                    .await?;
                self.hub.drop_member(organization, account).await;
                self.revoked(&revoked, &[account]).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::GetKeyWork { request } => Ok(ServerMessage::KeyWork {
                request,
                work: self.store.key_work(self.identity()?).await?,
            }),
            ClientMessage::Rotate {
                request,
                roots,
                keys,
            } => {
                let identity = self.identity()?;
                self.store.rotate(identity, roots.clone(), keys).await?;
                self.reannounce(identity, roots, true, BTreeSet::new())
                    .await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::CreateEscrow {
                request,
                escrow,
                keys,
            } => {
                self.store
                    .create_escrow(self.identity()?, escrow, keys)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::PinEscrow { request, sealed } => {
                self.store.pin_escrow(self.identity()?, sealed).await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListMembers { request } => Ok(ServerMessage::Members {
                request,
                members: self.store.list_members(self.identity()?).await?,
            }),
            ClientMessage::SetRole {
                request,
                account,
                role,
            } => {
                let identity = self.identity()?;
                self.store.set_role(identity, account, role).await?;
                if account != identity.account {
                    self.hub.drop_member(identity.organization, account).await;
                }
                self.key_work_changed(identity.organization, BTreeSet::new())
                    .await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::RemoveMember { request, account } => {
                let identity = self.identity()?;
                let revoked = self.store.remove_member(identity, account).await?;
                self.hub.drop_member(identity.organization, account).await;
                self.revoked(&revoked, &[account]).await;
                self.key_work_changed(identity.organization, BTreeSet::new())
                    .await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::Pair {
                request,
                to,
                payload,
            } => {
                let account = self.account()?;
                let message = ServerMessage::Paired {
                    from: self.client,
                    payload,
                };
                let sent = self
                    .hub
                    .send_account(account, self.client, to, &message)
                    .await;
                if to.is_some() && sent == 0 {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "that device is no longer connected".into(),
                    ));
                }
                Ok(ServerMessage::Ok { request })
            }
        }
    }

    async fn revoked(&self, revoked: &[contacts::Granted], accounts: &[Uuid]) {
        let mut organizations = BTreeSet::new();
        for granted in revoked {
            self.reannounce_in(
                granted.organization,
                vec![granted.block],
                true,
                granted.before.clone(),
            )
            .await;
            self.key_work_changed(granted.organization, granted.before.clone())
                .await;
            organizations.insert(granted.organization);
        }
        for organization in organizations {
            for account in accounts {
                self.evict_sessions(organization, *account).await;
            }
        }
    }

    async fn key_work_changed(&self, organization: Uuid, accounts: BTreeSet<Uuid>) {
        for (client, _) in self.hub.audience(organization, &accounts).await {
            if client != self.client {
                self.hub
                    .send_to(client, ServerMessage::KeyWorkChanged)
                    .await;
            }
        }
    }

    async fn evict_sessions(&self, organization: Uuid, account: Uuid) {
        for (client, identity) in self.hub.clients_of(organization, account).await {
            for block in self.registry.blocks_of(client).await {
                if self.store.read_block(identity, block).await.is_ok() {
                    continue;
                }
                self.hub.unwatch(block, client).await;
                if let Some(state) = self.registry.leave(block, client).await {
                    let mut told = state.participants.clone();
                    told.push(client);
                    self.hub
                        .send_all(
                            &told,
                            0,
                            &ServerMessage::SessionChanged {
                                block,
                                state: state.clone(),
                            },
                        )
                        .await;
                }
            }
        }
    }

    async fn contacts_changed(&self, accounts: &[Uuid]) {
        for account in accounts {
            self.hub
                .send_account(*account, self.client, None, &ServerMessage::ContactsChanged)
                .await;
        }
    }

    async fn reannounce(
        &self,
        identity: Identity,
        blocks: Vec<Uuid>,
        subtree: bool,
        before: BTreeSet<Uuid>,
    ) {
        self.reannounce_in(identity.organization, blocks, subtree, before)
            .await;
    }

    async fn reannounce_in(
        &self,
        organization: Uuid,
        blocks: Vec<Uuid>,
        subtree: bool,
        before: BTreeSet<Uuid>,
    ) {
        let Ok(mut accounts) = self.store.audience(organization, blocks.clone()).await else {
            return;
        };
        accounts.extend(before);
        let members = self.hub.audience(organization, &accounts).await;
        let identities = members.iter().map(|(_, member)| *member).collect();
        let Ok(views) = self
            .store
            .graph_views(organization, identities, blocks, subtree)
            .await
        else {
            return;
        };
        for ((client, _), messages) in members.into_iter().zip(views) {
            for (message, entry) in messages {
                self.hub.tell(client, message, entry).await;
            }
        }
    }

    async fn list(&self, listing: Listing) -> Result<Vec<BlockSummary>, ServerError> {
        let identity = self.identity()?;
        self.hub.list(self.client, listing).await;
        let blocks = match listing {
            Listing::Shared => self.store.list_shared(identity).await?,
            Listing::Children(parent) => self.store.list_children(identity, parent).await?,
            Listing::Deleted => self.store.list_deleted(identity).await?,
            Listing::Backrefs(block) => self.store.list_backrefs(identity, block).await?,
            Listing::References(block) => self.store.list_references(identity, block).await?,
            Listing::Block(block) => vec![self.store.read_block(identity, block).await?],
            Listing::OpenSettings(block) => self.store.list_open_settings(identity, block).await?,
        };
        self.hub.listed(self.client, listing, &blocks).await;
        Ok(blocks)
    }

    async fn announce(&self, block: Uuid, state: &be_protocol::SessionState) {
        self.hub
            .send_all(
                &state.participants,
                self.client,
                &ServerMessage::SessionChanged {
                    block,
                    state: state.clone(),
                },
            )
            .await;
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
