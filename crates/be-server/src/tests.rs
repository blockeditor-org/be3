use std::{path::PathBuf, time::Duration};

use be_commit::{Commit, CommitId, CommitStore};
use be_graph::BlockParent;
use be_protocol::{
    BlockSummary, ClientMessage, ErrorCode, HistoryEntry, ServerMessage, encode as encode_message,
};
use be_store::{ChunkerConfig, ContentKey, Hash, MemoryStore, ObjectStore, Vault};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message as WsMessage,
};
use uuid::Uuid;

use super::*;

mod a_detached_subtree_is_collected_and_its_objects_freed;
mod a_stale_publish_is_rejected_with_the_current_head;
mod a_watcher_is_told_when_the_head_moves;
mod an_unauthenticated_connection_cannot_touch_blocks;
mod blocks_publish_and_read_back_through_the_server;
mod shared_chunks_survive_until_the_last_commit_releases_them;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);

struct Harness {
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, receiver) = oneshot::channel();
        let data_dir = directory.clone();
        let handle = tokio::spawn(async move {
            let _ = serve_until_shutdown(listener, data_dir, receiver).await;
        });
        Self {
            url: format!("ws://{address}"),
            directory,
            shutdown: Some(shutdown),
            handle: Some(handle),
        }
    }

    async fn client(&self) -> TestClient {
        let (socket, _) = connect_async(&self.url).await.unwrap();
        TestClient {
            socket,
            next: 0,
            notifications: Vec::new(),
            token: String::new(),
        }
    }

    async fn member(&self, email: &str) -> TestClient {
        let mut client = self.client().await;
        client.register(email).await;
        client
    }

    async fn second_connection(&self, token: &str, workspace: Uuid) -> TestClient {
        let mut client = self.client().await;
        let response = client
            .send(|request| ClientMessage::Authenticate {
                request,
                token: token.to_owned(),
            })
            .await;
        assert!(
            matches!(response, ServerMessage::Authenticated { .. }),
            "{response:?}"
        );
        client.open(workspace).await;
        client
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct TestClient {
    socket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    next: u64,
    notifications: Vec<ServerMessage>,
    token: String,
}

impl TestClient {
    async fn send(&mut self, build: impl FnOnce(u64) -> ClientMessage) -> ServerMessage {
        self.next += 1;
        let request = self.next;
        self.socket
            .send(WsMessage::Binary(encode_message(&build(request)).unwrap()))
            .await
            .unwrap();
        loop {
            let message = tokio::time::timeout(Duration::from_secs(10), self.socket.next())
                .await
                .expect("the server answered in time")
                .expect("the connection stayed open")
                .unwrap();
            let WsMessage::Binary(bytes) = message else {
                continue;
            };
            let response: ServerMessage = be_protocol::decode(&bytes).unwrap();
            match response.request() {
                Some(id) if id == request => return response,
                Some(_) => panic!("the server answered a request that was not outstanding"),
                None => self.notifications.push(response),
            }
        }
    }

    async fn notification(&mut self) -> ServerMessage {
        if !self.notifications.is_empty() {
            return self.notifications.remove(0);
        }
        loop {
            let message = tokio::time::timeout(Duration::from_secs(10), self.socket.next())
                .await
                .expect("a notification arrived in time")
                .expect("the connection stayed open")
                .unwrap();
            if let WsMessage::Binary(bytes) = message {
                let response: ServerMessage = be_protocol::decode(&bytes).unwrap();
                if response.request().is_none() {
                    return response;
                }
            }
        }
    }

    async fn register(&mut self, email: &str) -> Uuid {
        let response = self
            .send(|request| ClientMessage::Register {
                request,
                email: email.into(),
                display_name: email.into(),
                password: "correct horse battery".into(),
            })
            .await;
        let ServerMessage::Authenticated { account, token, .. } = response else {
            panic!("registration failed: {response:?}");
        };
        self.token = token;
        account
    }

    async fn workspace(&mut self, name: &str) -> Uuid {
        let response = self
            .send(|request| ClientMessage::CreateWorkspace {
                request,
                name: name.into(),
            })
            .await;
        let ServerMessage::Workspaces { workspaces, .. } = response else {
            panic!("workspace creation failed: {response:?}");
        };
        let workspace = workspaces[0].id;
        self.open(workspace).await;
        workspace
    }

    async fn open(&mut self, workspace: Uuid) {
        let response = self
            .send(|request| ClientMessage::OpenWorkspace { request, workspace })
            .await;
        assert!(
            matches!(response, ServerMessage::WorkspaceOpened { .. }),
            "{response:?}"
        );
    }

    async fn create_block(&mut self, parent: BlockParent) -> Uuid {
        let block = Uuid::new_v4();
        let response = self
            .send(|request| ClientMessage::CreateBlock {
                request,
                block,
                content_type: CONTENT,
                parent,
            })
            .await;
        assert!(
            matches!(response, ServerMessage::Block { .. }),
            "{response:?}"
        );
        block
    }

    async fn upload(&mut self, store: &MemoryStore, hashes: &[Hash]) {
        for hash in hashes {
            let bytes = store.get(*hash).unwrap().unwrap();
            let response = self
                .send(|request| ClientMessage::PutObject { request, bytes })
                .await;
            assert!(
                matches!(response, ServerMessage::Stored { hash: stored, .. } if stored == *hash),
                "{response:?}"
            );
        }
    }

    async fn read_block(&mut self, block: Uuid) -> BlockSummary {
        let response = self
            .send(|request| ClientMessage::ReadBlock { request, block })
            .await;
        let ServerMessage::Block { block, .. } = response else {
            panic!("read failed: {response:?}");
        };
        block
    }

    async fn history(&mut self, block: Uuid) -> Vec<HistoryEntry> {
        let response = self
            .send(|request| ClientMessage::ListHistory { request, block })
            .await;
        let ServerMessage::History { entries, .. } = response else {
            panic!("history failed: {response:?}");
        };
        entries
    }
}

struct Author {
    store: MemoryStore,
    commits: CommitStore<MemoryStore>,
    account: Uuid,
}

impl Author {
    fn new(account: Uuid) -> Self {
        let store = MemoryStore::new();
        Self {
            commits: CommitStore::new(
                Vault::new(store.clone(), ContentKey::from_bytes([5; 32]))
                    .with_chunker(ChunkerConfig::SMALL),
            ),
            store,
            account,
        }
    }

    fn compose(&self, text: &[u8], time: i64, parent: Option<CommitId>) -> (CommitId, Vec<Hash>) {
        let manifest = self.commits.vault().write(CONTENT, text).unwrap();
        let chunks = manifest.chunk_hashes();
        let commit = self
            .commits
            .put(&Commit::new(manifest, self.account, time).with_parent(parent))
            .unwrap();
        (commit, chunks)
    }

    fn objects_of(&self, commit: CommitId, chunks: &[Hash]) -> Vec<Hash> {
        let mut hashes = vec![commit.hash()];
        hashes.extend_from_slice(chunks);
        hashes
    }
}
