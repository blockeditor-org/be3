use std::{
    error::Error,
    fmt, io,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use be_protocol::{
    ClientMessage, ErrorCode, MAX_FRAME_BYTES, ServerMessage, WorkspaceRole, decode, encode,
};
use be_session::Claim;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc},
};
use tokio_tungstenite::{
    accept_hdr_async_with_config,
    tungstenite::{
        Message,
        handshake::server::{Callback, ErrorResponse, Request, Response},
        protocol::WebSocketConfig,
    },
};
use uuid::Uuid;

pub mod blocks;
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
    Socket(String),
}

impl ServerError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Refused(code, _) => *code,
            Self::Corrupt | Self::Database(_) | Self::Store(_) | Self::Io(_) => ErrorCode::Storage,
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

#[derive(Clone, Copy, Debug, Default)]
pub struct ServerConfig {
    pub allow_registration: bool,
}

impl ServerConfig {
    pub const OPEN: Self = Self {
        allow_registration: true,
    };
}

const MAX_CONNECTIONS: usize = 1024;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const OUTBOUND_LIMIT: usize = 4096;
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

pub async fn serve_with_config(
    listener: TcpListener,
    data_dir: impl Into<PathBuf>,
    config: ServerConfig,
    shutdown: impl Future<Output = impl Sized>,
) -> Result<(), ServerError> {
    let store = Arc::new(ServerStore::open(data_dir.into())?);
    let hub = Arc::new(WatchHub::new());
    let registry = Arc::new(SessionRegistry::new());
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                store.checkpoint().await?;
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
                let _ = stream.set_nodelay(true);
                let store = Arc::clone(&store);
                let hub = Arc::clone(&hub);
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(stream, peer, store, hub, registry, config).await {
                        tracing::debug!(%peer, %error, "a connection ended with an error");
                    }
                    drop(slot);
                });
            }
        }
    }
}

struct Connection {
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
    client: u64,
    account: Option<Uuid>,
    token: Option<String>,
    identity: Option<Identity>,
    config: ServerConfig,
}

struct Forwarded(Arc<std::sync::Mutex<Option<String>>>);

impl Callback for Forwarded {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        *self.0.lock().unwrap() = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(|value| value.trim().to_owned());
        Ok(response)
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
    config: ServerConfig,
) -> Result<(), ServerError> {
    let forwarded = Arc::new(std::sync::Mutex::new(None));
    let handshake = accept_hdr_async_with_config(
        stream,
        Forwarded(Arc::clone(&forwarded)),
        Some(WebSocketConfig {
            max_frame_size: Some(MAX_FRAME_BYTES),
            max_message_size: Some(MAX_FRAME_BYTES),
            ..WebSocketConfig::default()
        }),
    );
    let socket = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake)
        .await
        .map_err(|_| ServerError::Socket("the websocket handshake timed out".into()))??;
    let client = forwarded
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| peer.ip().to_string());
    tracing::debug!(%client, "connected");
    serve_socket(socket, store, hub, registry, config).await
}

pub async fn serve_socket<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
    config: ServerConfig,
) -> Result<(), ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut sink, mut source) = socket.split();
    let (sender, mut outbound) = mpsc::channel(OUTBOUND_LIMIT);
    let mut pings = tokio::time::interval(PING_INTERVAL);
    let mut heard = Instant::now();
    let client = hub.register(sender).await;
    let mut connection = Connection {
        store,
        hub: Arc::clone(&hub),
        registry: Arc::clone(&registry),
        client,
        account: None,
        token: None,
        identity: None,
        config,
    };

    let outcome = async {
        loop {
            let frame = tokio::select! {
                notification = outbound.recv() => {
                    let Some(notification) = notification else { return Ok(()) };
                    sink.send(Message::Binary(
                        encode(&notification).map_err(|_| ServerError::Corrupt)?,
                    ))
                    .await?;
                    continue;
                }
                _ = pings.tick() => {
                    if heard.elapsed() > IDLE_TIMEOUT {
                        return Ok(());
                    }
                    sink.send(Message::Ping(Vec::new())).await?;
                    continue;
                }
                message = source.next() => message,
            };
            let Some(frame) = frame else { return Ok(()) };
            heard = Instant::now();
            match frame? {
                Message::Binary(bytes) => {
                    let response = match decode::<ClientMessage>(&bytes) {
                        Err(_) => ServerMessage::Failed {
                            request: 0,
                            code: ErrorCode::InvalidRequest,
                            message: "the frame is not a valid protocol message".into(),
                        },
                        Ok(request) => {
                            let id = request.request();
                            match connection.dispatch(request).await {
                                Ok(response) => response,
                                Err(error) => ServerMessage::Failed {
                                    request: id,
                                    code: error.code(),
                                    message: error.to_string(),
                                },
                            }
                        }
                    };
                    while let Ok(notification) = outbound.try_recv() {
                        sink.send(Message::Binary(
                            encode(&notification).map_err(|_| ServerError::Corrupt)?,
                        ))
                        .await?;
                    }
                    sink.send(Message::Binary(
                        encode(&response).map_err(|_| ServerError::Corrupt)?,
                    ))
                    .await?;
                }
                Message::Close(_) => return Ok(()),
                Message::Ping(payload) => sink.send(Message::Pong(payload)).await?,
                Message::Text(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }
    .await;

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
                "open a workspace before using it".into(),
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
        match message {
            ClientMessage::Register {
                request,
                email,
                display_name,
                password,
            } => {
                if !self.config.allow_registration {
                    return Err(ServerError::Refused(
                        ErrorCode::RegistrationDisabled,
                        "this server does not accept new accounts".into(),
                    ));
                }
                let (profile, token) = self
                    .store
                    .register(&email, &display_name, &password)
                    .await?;
                Ok(self.authenticated(request, profile, token).await)
            }
            ClientMessage::Login {
                request,
                email,
                password,
            } => {
                let (profile, token) = self
                    .store
                    .login(&email, &password)
                    .await
                    .inspect_err(|error| tracing::warn!(email, %error, "a sign-in failed"))?;
                Ok(self.authenticated(request, profile, token).await)
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
                workspace,
                email,
                role,
            } => {
                self.store
                    .invite(self.account()?, workspace, &email, role)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListInvitations { request } => Ok(ServerMessage::Invitations {
                request,
                invitations: self.store.list_invitations(self.account()?).await?,
            }),
            ClientMessage::RespondInvitation {
                request,
                invitation,
                accept,
            } => {
                self.store
                    .respond_invitation(self.account()?, invitation, accept)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListWorkspaces { request } => Ok(ServerMessage::Workspaces {
                request,
                workspaces: self.store.list_workspaces(self.account()?).await?,
            }),
            ClientMessage::CreateWorkspace { request, name } => {
                let workspace = self.store.create_workspace(self.account()?, &name).await?;
                Ok(ServerMessage::Workspaces {
                    request,
                    workspaces: vec![workspace],
                })
            }
            ClientMessage::OpenWorkspace { request, workspace } => {
                let account = self.account()?;
                let role = self.store.membership(account, workspace).await?;
                self.identity = Some(Identity {
                    account,
                    workspace,
                    role,
                });
                self.hub
                    .join_workspace(
                        self.client,
                        Identity {
                            account,
                            workspace,
                            role,
                        },
                    )
                    .await;
                Ok(ServerMessage::WorkspaceOpened {
                    request,
                    workspace,
                    role,
                })
            }

            ClientMessage::PutObject { request, bytes } => {
                self.identity()?;
                Ok(ServerMessage::Stored {
                    request,
                    hash: self.store.put_object(&bytes)?,
                })
            }
            ClientMessage::GetObject { request, hash } => {
                self.identity()?;
                Ok(ServerMessage::Object {
                    request,
                    bytes: self.store.get_object(hash)?,
                })
            }
            ClientMessage::GetObjectRange {
                request,
                hash,
                offset,
                length,
            } => {
                self.identity()?;
                Ok(ServerMessage::Object {
                    request,
                    bytes: self.store.get_object_range(hash, offset, length)?,
                })
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
            } => {
                let identity = self.identity()?;
                let block = self
                    .store
                    .create_block(identity, block, content_type, parent, metadata)
                    .await?;
                self.reannounce(identity, vec![block.id], false).await;
                Ok(ServerMessage::Block { request, block })
            }
            ClientMessage::ListBlocks { request } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.store.list_blocks(self.identity()?).await?,
            }),
            ClientMessage::SetMetadata {
                request,
                block,
                metadata,
            } => {
                let identity = self.identity()?;
                let block = self.store.set_metadata(identity, block, metadata).await?;
                self.reannounce(identity, vec![block.id], false).await;
                Ok(ServerMessage::Block { request, block })
            }
            ClientMessage::Publish {
                request,
                block,
                commit,
                expected,
                chunks,
                time,
                pinned,
                references_added,
                references_removed,
            } => {
                let identity = self.identity()?;
                let relinked = !references_added.is_empty() || !references_removed.is_empty();
                let outcome = self
                    .store
                    .publish(
                        identity,
                        block,
                        commit,
                        expected,
                        chunks,
                        time,
                        pinned,
                        references_added,
                        references_removed,
                    )
                    .await?;
                if relinked && matches!(outcome, PublishOutcome::Published(_)) {
                    self.reannounce(identity, vec![block], false).await;
                }
                Ok(match outcome {
                    PublishOutcome::Published(head) => {
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
                block: self.store.read_block(self.identity()?, block).await?,
            }),
            ClientMessage::SetParent {
                request,
                block,
                parent,
            } => {
                let identity = self.identity()?;
                self.store.set_parent(identity, block, parent).await?;
                self.reannounce(identity, vec![block], true).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListChildren { request, parent } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.store.list_children(self.identity()?, parent).await?,
            }),
            ClientMessage::ListBackrefs { request, block } => Ok(ServerMessage::Blocks {
                request,
                blocks: self.store.list_backrefs(self.identity()?, block).await?,
            }),
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
                self.store.delete_block(identity, block).await?;
                self.reannounce(identity, vec![block], true).await;
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
                        .announce(
                            identity.workspace,
                            ServerMessage::BlockRemoved { block: *block },
                        )
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
                self.store
                    .set_access(identity, block, account, access)
                    .await?;
                self.reannounce(identity, vec![block], true).await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListAccess { request, block } => Ok(ServerMessage::AccessList {
                request,
                entries: self.store.list_access(self.identity()?, block).await?,
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
            ClientMessage::GetKeys { request } => {
                let (recovery, sealed) = self.store.keys(self.account()?).await?;
                Ok(ServerMessage::Keys {
                    request,
                    recovery,
                    sealed,
                })
            }
            ClientMessage::SetRecoveryKey {
                request,
                public,
                sealed,
            } => {
                self.store
                    .set_recovery_key(self.account()?, public, sealed)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::PutWorkspaceKey {
                request,
                workspace,
                account,
                sealed,
            } => {
                self.store
                    .put_workspace_key(self.account()?, workspace, account, sealed)
                    .await?;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::ListMemberKeys { request, workspace } => {
                let members = self.store.member_keys(self.account()?, workspace).await?;
                Ok(ServerMessage::MemberKeys { request, members })
            }
            ClientMessage::Pair {
                request,
                to,
                workspace,
                payload,
            } => {
                let account = self.account()?;
                self.store.membership(account, workspace).await?;
                let message = ServerMessage::Paired {
                    from: self.client,
                    workspace,
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

    async fn reannounce(&self, identity: Identity, blocks: Vec<Uuid>, subtree: bool) {
        let members = self.hub.members(identity.workspace).await;
        let identities = members.iter().map(|(_, member)| *member).collect();
        let Ok(views) = self
            .store
            .graph_views(identity.workspace, identities, blocks, subtree)
            .await
        else {
            return;
        };
        for ((client, _), messages) in members.into_iter().zip(views) {
            for message in messages {
                self.hub.send_to(client, message).await;
            }
        }
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

pub async fn add_account(
    data_dir: impl Into<PathBuf>,
    email: &str,
    display_name: &str,
    password: &str,
    workspace: &str,
) -> Result<(Uuid, Uuid), ServerError> {
    let store = ServerStore::open(data_dir.into())?;
    let (profile, _) = store.register(email, display_name, password).await?;
    let account = profile.account;
    let workspace = store.create_workspace(account, workspace).await?;
    store
        .add_member(workspace.id, account, WorkspaceRole::Administrator)
        .await?;
    Ok((account, workspace.id))
}

#[cfg(test)]
mod tests;
