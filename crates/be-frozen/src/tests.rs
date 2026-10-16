use std::path::PathBuf;

use be_cli::tracker::Tracker;
use be_client::{Credentials, Peer, PeerConfig};
use be_store::MemoryStore;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use uuid::Uuid;

use crate::scene;

mod a_tracker_downloaded_as_a_block_file_uploads_and_reads_back;

const TRACKER: &[u8] = include_bytes!("fixtures/tracker.block");

struct Server {
    url: String,
    data: PathBuf,
    stop: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
}

impl Server {
    async fn start() -> Self {
        build_id::install("frozen");
        let data = std::env::temp_dir().join(format!("be-frozen-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel::<()>();
        let served = data.clone();
        let handle = tokio::spawn(async move {
            be_server::serve_with_config(listener, served, be_server::ServerConfig::OPEN, stopped)
                .await
                .expect("the server starts");
        });
        Self {
            url,
            data,
            stop: Some(stop),
            handle,
        }
    }

    async fn owner(&self) -> Tracker<MemoryStore> {
        Tracker::new(
            Peer::connect(
                PeerConfig::new(
                    be_client::socket_url(&self.url),
                    be_keys::RecoveryPhrase::generate().secret(),
                    Credentials::Register {
                        email: scene::OWNER_EMAIL.into(),
                        display_name: scene::OWNER_NAME.into(),
                        password: scene::OWNER_PASSWORD.into(),
                    },
                ),
                MemoryStore::new(),
            )
            .await
            .unwrap(),
        )
    }

    async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.handle.abort();
        let _ = self.handle.await;
        let _ = std::fs::remove_dir_all(&self.data);
    }
}
