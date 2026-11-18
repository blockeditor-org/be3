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
        mut config: ServerConfig,
        mail: Arc<dyn mail::Delivery>,
    ) -> Self {
        config.lies.get_or_insert_with(crate::Lies::default);
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
            if let Err(error) =
                crate::serve_with_mail(listener, directory, config, mail, stopped).await
            {
                panic!("the test server stopped: {error}");
            }
        }));
    }

    /// Makes this server lie to `account`: every page it sends that account's devices is
    /// rewritten by `lie` (`Lies`), as a server that changes what it says would.
    pub fn lie(
        &self,
        account: Uuid,
        lie: impl Fn(&mut be_block_protocol::Page) + Send + Sync + 'static,
    ) {
        self.config
            .lies
            .as_ref()
            .expect("a running test server can lie")
            .to(account, lie);
    }

    pub async fn register(&self, email: &str, name: &str) -> String {
        ServerStore::open(&self.directory)
            .unwrap_or_else(|error| panic!("the store beside the server did not open: {error}"))
            .register(email, name)
            .await
            .unwrap_or_else(|error| panic!("registering {email} failed: {error}"))
            .1
    }

    /// Restarts the server on the same data with `config`, as an operator changing its flags.
    pub async fn restart_with(&mut self, config: ServerConfig) {
        let lies = self.config.lies.take();
        self.config = ServerConfig { lies, ..config };
        self.restart().await;
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
