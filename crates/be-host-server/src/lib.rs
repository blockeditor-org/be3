//! The block server be-host runs in its own process for a local account, and tests start to
//! sign in against: be-server on a thread of its own, stopped when dropped.

use std::{error::Error, net::TcpListener as StdTcpListener, path::PathBuf, thread};

use tokio::net::TcpListener;

#[cfg(not(target_arch = "wasm32"))]
pub mod testing;

pub struct EmbeddedServer {
    pub url: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start_embedded_server(
    data_dir: PathBuf,
) -> Result<EmbeddedServer, Box<dyn Error + Send + Sync>> {
    start_embedded_server_at(
        "127.0.0.1:0",
        data_dir,
        embedded_config(be_server::push::PushConfig::DEFAULT),
    )
}

/// What an embedded server serves with: open to anyone, with no account quota, pushing where
/// `push` says (tests that serve a push service on loopback allow local endpoints).
pub fn embedded_config(push: be_server::push::PushConfig) -> be_server::ServerConfig {
    be_server::ServerConfig {
        account_quota: None,
        push,
        ..be_server::ServerConfig::OPEN
    }
}

/// A server at `address`, serving with `config` (`embedded_config`, or one a test changed).
pub fn start_embedded_server_at(
    address: &str,
    data_dir: PathBuf,
    config: be_server::ServerConfig,
) -> Result<EmbeddedServer, Box<dyn Error + Send + Sync>> {
    let listener = StdTcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let thread = thread::Builder::new()
        .name("be-host-server".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create embedded block server runtime");
            runtime.block_on(async move {
                let listener = TcpListener::from_std(listener)
                    .expect("failed to initialize embedded block server listener");
                let shutdown = async {
                    let _ = shutdown_receiver.await;
                };
                if let Err(error) = be_server::serve_with_config(
                    listener,
                    data_dir,
                    config,
                    shutdown,
                )
                .await
                {
                    panic!("embedded block server stopped: {error}");
                }
            });
        })?;
    Ok(EmbeddedServer {
        url: format!("http://{address}"),
        shutdown: Some(shutdown_sender),
        thread: Some(thread),
    })
}
