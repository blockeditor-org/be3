use std::{error::Error, fmt, io, path::PathBuf, sync::Arc};

use be_protocol::{
    ClientMessage, ErrorCode, MAX_FRAME_BYTES, ServerMessage, WorkspaceRole, decode, encode,
};
use be_session::Claim;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::{
    accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
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

#[derive(Clone, Debug)]
pub struct Adopted {
    pub account: Uuid,
    pub display_name: String,
}

pub struct Hosted {
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
}

impl Hosted {
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Self, ServerError> {
        Ok(Self {
            store: Arc::new(ServerStore::open(data_dir.into())?),
            hub: Arc::new(WatchHub::new()),
            registry: Arc::new(SessionRegistry::new()),
        })
    }

    pub async fn adopt(
        &self,
        account: Uuid,
        email: &str,
        display_name: &str,
        workspace: Uuid,
        workspace_name: &str,
        role: WorkspaceRole,
    ) -> Result<(), ServerError> {
        self.store
            .adopt(
                account,
                email,
                display_name,
                workspace,
                workspace_name,
                role,
            )
            .await?;
        Ok(())
    }

    pub async fn serve<S>(
        &self,
        socket: tokio_tungstenite::WebSocketStream<S>,
        adopted: Adopted,
    ) -> Result<(), ServerError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send,
    {
        serve_socket(
            socket,
            Arc::clone(&self.store),
            Arc::clone(&self.hub),
            Arc::clone(&self.registry),
            Some(adopted),
        )
        .await
    }
}

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

pub async fn serve(listener: TcpListener, data_dir: impl Into<PathBuf>) -> Result<(), ServerError> {
    let (_shutdown, receiver) = oneshot::channel();
    serve_until_shutdown(listener, data_dir, receiver).await
}

pub async fn serve_until_shutdown(
    listener: TcpListener,
    data_dir: impl Into<PathBuf>,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), ServerError> {
    let store = Arc::new(ServerStore::open(data_dir.into())?);
    let hub = Arc::new(WatchHub::new());
    let registry = Arc::new(SessionRegistry::new());
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let store = Arc::clone(&store);
                let hub = Arc::clone(&hub);
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = handle_connection(stream, store, hub, registry).await;
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
    identity: Option<Identity>,
    adopted: Option<Adopted>,
}

async fn handle_connection(
    stream: TcpStream,
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
) -> Result<(), ServerError> {
    let socket = accept_async_with_config(
        stream,
        Some(WebSocketConfig {
            max_frame_size: Some(MAX_FRAME_BYTES),
            max_message_size: Some(MAX_FRAME_BYTES),
            ..WebSocketConfig::default()
        }),
    )
    .await?;
    serve_socket(socket, store, hub, registry, None).await
}

pub async fn serve_socket<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    registry: Arc<SessionRegistry>,
    adopted: Option<Adopted>,
) -> Result<(), ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut sink, mut source) = socket.split();
    let (sender, mut outbound) = mpsc::unbounded_channel();
    let client = hub.register(sender).await;
    let mut connection = Connection {
        store,
        hub: Arc::clone(&hub),
        registry: Arc::clone(&registry),
        client,
        account: None,
        identity: None,
        adopted,
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
                message = source.next() => message,
            };
            let Some(frame) = frame else { return Ok(()) };
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
                let (account, display_name, token) = self
                    .store
                    .register(&email, &display_name, &password)
                    .await?;
                self.account = Some(account);
                Ok(ServerMessage::Authenticated {
                    request,
                    account,
                    display_name,
                    token,
                })
            }
            ClientMessage::Login {
                request,
                email,
                password,
            } => {
                let (account, display_name, token) = self.store.login(&email, &password).await?;
                self.account = Some(account);
                Ok(ServerMessage::Authenticated {
                    request,
                    account,
                    display_name,
                    token,
                })
            }
            ClientMessage::Authenticate { request, token } => {
                let (account, display_name) = self.store.resolve_token(&token).await?;
                self.account = Some(account);
                Ok(ServerMessage::Authenticated {
                    request,
                    account,
                    display_name,
                    token,
                })
            }
            ClientMessage::Adopt { request } => {
                let Some(adopted) = self.adopted.clone() else {
                    return Err(ServerError::Refused(
                        ErrorCode::NotAuthenticated,
                        "this connection carries no identity to adopt".into(),
                    ));
                };
                let token = self.store.issue_session(adopted.account).await?;
                self.account = Some(adopted.account);
                Ok(ServerMessage::Authenticated {
                    request,
                    account: adopted.account,
                    display_name: adopted.display_name,
                    token,
                })
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
                self.hub.join_workspace(self.client, workspace).await;
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
                self.changed(identity, block.clone()).await;
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
                self.changed(identity, block.clone()).await;
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
                    self.reannounce(identity, block).await;
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
                self.reannounce(identity, block).await;
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
                self.reannounce(identity, block).await;
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
                self.reannounce(identity, block).await;
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
        }
    }

    async fn changed(&self, identity: Identity, block: be_protocol::BlockSummary) {
        self.hub
            .announce(identity.workspace, ServerMessage::BlockChanged { block })
            .await;
    }

    async fn reannounce(&self, identity: Identity, block: Uuid) {
        if let Ok(summary) = self.store.read_block(identity, block).await {
            self.changed(identity, summary).await;
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
    let (account, _, _) = store.register(email, display_name, password).await?;
    let workspace = store.create_workspace(account, workspace).await?;
    store
        .add_member(workspace.id, account, WorkspaceRole::Administrator)
        .await?;
    Ok((account, workspace.id))
}

#[cfg(test)]
mod tests;
