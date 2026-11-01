use std::{fs, io::Read, path::PathBuf};

use be_cli::tracker::Tracker;
use be_client::{Credentials, Peer, PeerConfig};
use be_server::Running;
use be_store::MemoryStore;
use uuid::Uuid;

use crate::scene;
use crate::snapshots::{SNAPSHOTS, Snapshot};

mod a_tracker_downloaded_as_a_block_file_uploads_and_reads_back;
mod every_snapshot_of_a_servers_data_still_serves_what_it_held;

const TRACKER: &[u8] = include_bytes!("fixtures/tracker.block");

pub(crate) fn sources() -> be_cli::tracker::Sources {
    Box::new(first_party_types::sources)
}

async fn start(data: Option<PathBuf>) -> Running {
    build_id::install("frozen");
    match data {
        Some(data) => Running::at(data, be_server::ServerConfig::OPEN).await,
        None => Running::start(be_server::ServerConfig::OPEN).await,
    }
}

async fn owner(server: &Running) -> Tracker<MemoryStore> {
    Tracker::new(
        Peer::connect(
            PeerConfig::new(
                be_client::socket_url(&server.url),
                be_keys::RecoveryPhrase::generate().secret(),
                Credentials::Token(server.register(scene::OWNER_EMAIL, scene::OWNER_NAME).await),
            ),
            MemoryStore::new(),
        )
        .await
        .unwrap(),
        sources(),
    )
}

struct Unpacked {
    root: PathBuf,
    data: PathBuf,
    backup: PathBuf,
}

impl Drop for Unpacked {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unpack(snapshot: &Snapshot) -> Unpacked {
    let root = std::env::temp_dir().join(format!("be-frozen-{}-{}", snapshot.name, Uuid::new_v4()));
    let unpacked = Unpacked {
        data: root.join("data"),
        backup: root.join("backup"),
        root,
    };
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(snapshot.files)).unwrap();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.enclosed_name().unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        let mut targets = vec![unpacked.root.join(&name)];
        if let Ok(object) = name.strip_prefix("data")
            && object.starts_with("objects")
        {
            targets.push(unpacked.backup.join(object));
        }
        for target in targets {
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, &bytes).unwrap();
        }
    }
    unpacked
}

fn id(text: &str) -> Uuid {
    Uuid::parse_str(text).unwrap()
}
