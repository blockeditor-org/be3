use std::path::{Path, PathBuf};
use workspace_ui_types::{FileContent, FolderContent, WorkspaceContent};

use be_block::{BlockContent, BlockMetadata, EXECUTABLE, LiveEdit};
use be_client::{Credentials, Peer, PeerConfig};
use be_graph::BlockParent;
use be_store::MemoryStore;
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

use crate::{Report, Target};
use infinite_canvas_types::CanvasContent;
use text_block_types::{TextBlock, TextContent};

mod a_file_name_must_work_on_every_system;
mod a_folder_is_written_as_files_and_directories;
mod a_rename_on_disk_moves_the_same_block;
mod a_skipped_block_is_never_deleted_or_replaced;
mod deletions_reach_the_other_side;
mod edits_on_both_sides_merge_and_clashes_keep_both;
mod files_made_on_disk_become_blocks;
mod ignored_files_stay_off_the_server;
mod the_watcher_syncs_both_ways;

struct Harness {
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    url: String,
}

impl Harness {
    async fn start() -> Self {
        build_id::install("test");
        let directory = std::env::temp_dir().join(format!("be-sync-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, receiver) = oneshot::channel();
        let data = directory.join("server");
        let handle = tokio::spawn(async move {
            let _ = be_server::serve_with_config(
                listener,
                data,
                be_server::ServerConfig::OPEN,
                receiver,
            )
            .await;
        });
        Self {
            url: format!("http://{address}"),
            directory,
            shutdown: Some(shutdown),
            handle: Some(handle),
        }
    }

    async fn peer(&self) -> Peer<MemoryStore> {
        let token = be_server::ServerStore::open(self.directory.join("server"))
            .unwrap()
            .register(
                &format!("{}@example.com", Uuid::new_v4().simple()),
                "Owner",
                "correct horse battery",
            )
            .await
            .unwrap()
            .1;
        Peer::connect(
            PeerConfig::new(
                be_client::socket_url(&self.url),
                be_keys::RecoveryPhrase::generate().secret(),
                Credentials::Token(token),
            ),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    fn checkout(&self, name: &str) -> PathBuf {
        let path = self.directory.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
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

async fn workspace(peer: &Peer<MemoryStore>) -> Uuid {
    let id = Uuid::now_v7();
    peer.create_block(
        id,
        WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named("Code"),
    )
    .await
    .unwrap();
    id
}

async fn folder(peer: &Peer<MemoryStore>, parent: Uuid, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    peer.create_block(
        id,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(parent),
        &BlockMetadata::named(name),
    )
    .await
    .unwrap();
    list_in(peer, parent, id).await;
    id
}

async fn list_in(peer: &Peer<MemoryStore>, parent: Uuid, child: Uuid) {
    relist(peer, parent, be_block::ChildChange::Add(child)).await;
}

async fn relist(peer: &Peer<MemoryStore>, parent: Uuid, change: be_block::ChildChange) {
    let summary = peer.summary(parent).await.unwrap();
    match summary.content_type == WorkspaceContent::CONTENT_TYPE {
        true => relist_in::<WorkspaceContent>(peer, parent, change).await,
        false => relist_in::<FolderContent>(peer, parent, change).await,
    }
}

async fn relist_in<C: BlockContent + LiveEdit + Default>(
    peer: &Peer<MemoryStore>,
    parent: Uuid,
    change: be_block::ChildChange,
) {
    let mut content = peer.open::<C>(parent).await.unwrap().unwrap_or_default();
    let head = peer.local_head(parent);
    for operation in content.child_operations(change).unwrap() {
        content.apply(&operation);
    }
    peer.save(parent, &content, head).await.unwrap();
}

async fn block<C: BlockContent>(
    peer: &Peer<MemoryStore>,
    parent: Uuid,
    name: &str,
    content: &C,
) -> Uuid {
    let id = Uuid::now_v7();
    peer.create_block(
        id,
        C::CONTENT_TYPE,
        BlockParent::Block(parent),
        &BlockMetadata::named(name),
    )
    .await
    .unwrap();
    peer.save(id, content, None).await.unwrap();
    list_in(peer, parent, id).await;
    id
}

async fn text_block(peer: &Peer<MemoryStore>, parent: Uuid, name: &str, text: &str) -> Uuid {
    block(peer, parent, name, &TextBlock::of(text)).await
}

async fn types() -> be_types_host::Modules {
    be_types_host::started(first_party_types::sources(), be_client::Clock::real()).await
}

async fn sync(peer: &Peer<MemoryStore>, directory: &Path, root: Uuid) -> Report {
    crate::sync(
        peer,
        &mut types().await,
        directory,
        &Target { root: Some(root) },
    )
    .await
    .unwrap()
}

#[derive(Debug)]
struct Listed {
    id: Uuid,
    name: String,
    block_type: Uuid,
    executable: bool,
}

async fn children(peer: &Peer<MemoryStore>, parent: Uuid) -> Vec<Listed> {
    let summaries = peer.children(BlockParent::Block(parent)).await.unwrap();
    peer.learn(&summaries).await.unwrap();
    let mut listed: Vec<Listed> = summaries
        .iter()
        .map(|summary| {
            let metadata = peer.metadata(summary).unwrap();
            Listed {
                id: summary.id,
                name: metadata.name().unwrap_or_default(),
                block_type: summary.content_type,
                executable: metadata.get(&EXECUTABLE).unwrap_or(false),
            }
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name));
    listed
}

async fn child(peer: &Peer<MemoryStore>, parent: Uuid, name: &str) -> Listed {
    children(peer, parent)
        .await
        .into_iter()
        .find(|listed| listed.name == name)
        .unwrap_or_else(|| panic!("no block called {name}"))
}

async fn text_of(peer: &Peer<MemoryStore>, id: Uuid) -> String {
    TextBlock::text(&peer.open::<TextContent>(id).await.unwrap().unwrap())
}

async fn rewrite(peer: &Peer<MemoryStore>, id: Uuid, text: &str) {
    let mut content = peer.open::<TextContent>(id).await.unwrap().unwrap();
    let head = peer.local_head(id);
    let edit = TextBlock::rewrite(&content, 7, text.as_bytes()).unwrap();
    content.apply(&edit);
    peer.save(id, &content, head).await.unwrap();
}

fn read(directory: &Path, path: &str) -> String {
    std::fs::read_to_string(directory.join(path)).unwrap()
}

fn write(directory: &Path, path: &str, text: &str) {
    let path = directory.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
