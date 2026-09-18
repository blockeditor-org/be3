use std::{error::Error, fmt, io, path::PathBuf, sync::Arc};

use be_protocol::{
    ClientMessage, ErrorCode, MAX_FRAME_BYTES, ServerMessage, WorkspaceRole, decode, encode,
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
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
pub mod store;
pub mod watch;

pub use blocks::PublishOutcome;
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
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let store = Arc::clone(&store);
                let hub = Arc::clone(&hub);
                tokio::spawn(async move {
                    let _ = handle_connection(stream, store, hub).await;
                });
            }
        }
    }
}

struct Connection {
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
    client: u64,
    account: Option<Uuid>,
    identity: Option<Identity>,
}

async fn handle_connection(
    stream: TcpStream,
    store: Arc<ServerStore>,
    hub: Arc<WatchHub>,
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
    let (mut sink, mut source) = socket.split();
    let (sender, mut outbound) = mpsc::unbounded_channel();
    let client = hub.register(sender).await;
    let mut connection = Connection {
        store,
        hub: Arc::clone(&hub),
        client,
        account: None,
        identity: None,
    };

    loop {
        tokio::select! {
            Some(notification) = outbound.recv() => {
                sink.send(Message::Binary(encode(&notification).map_err(|_| ServerError::Corrupt)?)).await?;
            }
            Some(message) = source.next() => {
                match message? {
                    Message::Binary(bytes) => {
                        let Ok(request) = decode::<ClientMessage>(&bytes) else {
                            sink.send(Message::Binary(encode(&ServerMessage::Failed {
                                request: 0,
                                code: ErrorCode::InvalidRequest,
                                message: "the frame is not a valid protocol message".into(),
                            }).map_err(|_| ServerError::Corrupt)?)).await?;
                            continue;
                        };
                        let id = request.request();
                        let response = match connection.dispatch(request).await {
                            Ok(response) => response,
                            Err(error) => ServerMessage::Failed {
                                request: id,
                                code: error.code(),
                                message: error.to_string(),
                            },
                        };
                        sink.send(Message::Binary(encode(&response).map_err(|_| ServerError::Corrupt)?)).await?;
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => sink.send(Message::Pong(payload)).await?,
                    Message::Text(_) | Message::Pong(_) | Message::Frame(_) => {}
                }
            }
            else => break,
        }
    }

    hub.remove(client).await;
    Ok(())
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
            } => Ok(ServerMessage::Block {
                request,
                block: self
                    .store
                    .create_block(self.identity()?, block, content_type, parent)
                    .await?,
            }),
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
                self.store
                    .set_parent(self.identity()?, block, parent)
                    .await?;
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
                self.store.delete_block(self.identity()?, block).await?;
                self.hub
                    .broadcast(block, self.client, ServerMessage::BlockDeleted { block })
                    .await;
                Ok(ServerMessage::Ok { request })
            }
            ClientMessage::CollectDetached { request } => {
                let collected = self.store.collect_detached(self.identity()?).await?;
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
                self.store
                    .set_access(self.identity()?, block, account, access)
                    .await?;
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
        }
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
