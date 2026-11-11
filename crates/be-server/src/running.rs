use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use uuid::Uuid;

use crate::{ServerConfig, ServerStore, mail};

pub struct Running {
    pub address: SocketAddr,
    pub url: String,
    pub directory: PathBuf,
    config: ServerConfig,
    mail: Arc<dyn mail::Delivery>,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl Running {
    pub const PASSWORD: &str = "correct horse battery";

    pub async fn start(config: ServerConfig) -> Self {
        Self::at(
            std::env::temp_dir().join(format!("be-server-{}", Uuid::new_v4())),
            config,
        )
        .await
    }

    pub async fn at(directory: PathBuf, config: ServerConfig) -> Self {
        Self::with_mail(directory, config, Arc::new(mail::LogDelivery)).await
    }

    pub async fn with_mail(
        directory: PathBuf,
        config: ServerConfig,
        mail: Arc<dyn mail::Delivery>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut running = Self {
            address,
            url: format!("http://{address}"),
            directory,
            config,
            mail,
            shutdown: None,
            handle: None,
        };
        running.serve(listener);
        running
    }

    fn serve(&mut self, listener: TcpListener) {
        let (shutdown, stopped) = oneshot::channel();
        let (directory, config, mail) = (
            self.directory.clone(),
            self.config.clone(),
            self.mail.clone(),
        );
        self.shutdown = Some(shutdown);
        self.handle = Some(tokio::spawn(async move {
            let _ = crate::serve_with_mail(listener, directory, config, mail, stopped).await;
        }));
    }

    pub async fn register(&self, email: &str, name: &str) -> String {
        ServerStore::open(&self.directory)
            .unwrap()
            .register(email, name, Self::PASSWORD)
            .await
            .unwrap()
            .1
    }

    pub async fn restart(&mut self) {
        self.halt().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        self.address = listener.local_addr().unwrap();
        self.url = format!("http://{}", self.address);
        self.serve(listener);
    }

    async fn halt(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    pub async fn stop(mut self) {
        self.halt().await;
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
