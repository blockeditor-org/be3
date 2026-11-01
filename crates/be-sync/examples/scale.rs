use std::path::{Path, PathBuf};
use std::time::Instant;
use workspace_ui_types::WorkspaceContent;

use be_block::{BlockContent, BlockMetadata};
use be_client::{Credentials, Peer, PeerConfig};
use be_graph::BlockParent;
use be_store::FileStore;
use be_sync::Target;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(source), Some(work)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: scale DIRECTORY WORK-DIRECTORY (DIRECTORY gets a .be directory)");
        std::process::exit(2);
    };
    let (source, work) = (PathBuf::from(source), PathBuf::from(work));
    build_id::install("scale");
    let server = be_server::Running::at(work.join("server"), be_server::ServerConfig::OPEN).await;
    let url = server.url.clone();
    let token = server.register("scale@example.com", "Scale").await;
    let phrase = be_keys::RecoveryPhrase::generate();
    let organization = std::sync::Mutex::new(None);
    let connect = async |cache: &str| {
        let chosen = *organization.lock().unwrap();
        let peer = Peer::connect(
            PeerConfig::new(
                be_client::http_url(&url),
                phrase.secret(),
                Credentials::Token(token.clone()),
            )
            .organization(chosen),
            FileStore::open(work.join(cache)).unwrap(),
        )
        .await
        .unwrap();
        *organization.lock().unwrap() = Some(peer.organization());
        peer
    };
    let peer = connect("cache").await;
    let root = Uuid::now_v7();
    peer.create_block(
        root,
        WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named("Code"),
    )
    .await
    .unwrap();
    let target = Target { root: Some(root) };
    let mut types =
        be_types_host::started(first_party_types::sources(), be_client::Clock::real()).await;
    let mut time = async |label: &str, peer: &Peer<FileStore>, directory: &Path| {
        let started = Instant::now();
        let report = be_sync::sync(peer, &mut types, directory, &target)
            .await
            .unwrap();
        println!(
            "{label}: {:.2?}\n  {}",
            started.elapsed(),
            report.to_string().replace('\n', "\n  ")
        );
    };
    time("first upload", &peer, &source).await;
    time("no-op resync, same connection", &peer, &source).await;
    let fresh = connect("cache").await;
    let started = Instant::now();
    let listed = be_sync::remote::list(&fresh, root).await.unwrap();
    println!(
        "listing {} blocks, new connection: {:.2?}",
        listed.len(),
        started.elapsed()
    );
    let fresh = connect("cache").await;
    time("no-op resync, new connection", &fresh, &source).await;
    let started = Instant::now();
    let walked = be_sync::disk::walk(&source).unwrap();
    println!(
        "walking {} paths: {:.2?}",
        walked.found.len(),
        started.elapsed()
    );
    let edited = walked
        .found
        .keys()
        .find(|path| path.ends_with(".md"))
        .cloned()
        .unwrap();
    let mut text = std::fs::read_to_string(source.join(&edited)).unwrap();
    text.push_str("\nOne more line.\n");
    std::fs::write(source.join(&edited), text).unwrap();
    time("one file changed", &fresh, &source).await;
    let elsewhere = connect("elsewhere-cache").await;
    time(
        "first download, empty cache",
        &elsewhere,
        &work.join("clone"),
    )
    .await;
    time(
        "no-op resync of the download",
        &elsewhere,
        &work.join("clone"),
    )
    .await;
}
