use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use be_client::Connection;
use be_protocol::{ClientMessage, ServerMessage};
use be_server::ServerConfig;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use uuid::Uuid;

use crate::snapshots::{SNAPSHOTS, Snapshot};

mod every_snapshot_of_a_servers_data_still_serves_what_it_held;

struct Server {
    url: String,
    stop: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
}

impl Server {
    async fn start(data: &Path) -> Self {
        build_id::install("frozen");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel::<()>();
        let data = data.to_path_buf();
        let handle = tokio::spawn(async move {
            be_server::serve_with_config(listener, data, ServerConfig::OPEN, stopped)
                .await
                .expect("the server opens the data it was given");
        });
        Self {
            url,
            stop: Some(stop),
            handle,
        }
    }

    fn socket(&self) -> String {
        be_client::socket_url(&self.url)
    }

    async fn connect(&self) -> std::sync::Arc<Connection> {
        Connection::connect(&self.socket()).await.unwrap()
    }

    async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.handle.abort();
        let _ = self.handle.await;
    }
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
    let write = |path: PathBuf, bytes: &[u8]| {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    };
    for (path, bytes) in snapshot.data {
        write(unpacked.data.join(path), bytes);
        if path.starts_with("objects/") {
            write(unpacked.backup.join(path), bytes);
        }
    }
    write(unpacked.backup.join(snapshot.backup.0), snapshot.backup.1);
    unpacked
}

async fn request(
    connection: &Connection,
    build: impl FnOnce(u64) -> ClientMessage,
) -> ServerMessage {
    tokio::time::timeout(Duration::from_secs(30), connection.request(build))
        .await
        .expect("the server answered in time")
        .unwrap()
}

fn id(text: &str) -> Uuid {
    Uuid::parse_str(text).unwrap()
}

fn key(hex: &str) -> [u8; 32] {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect();
    bytes.try_into().unwrap()
}
