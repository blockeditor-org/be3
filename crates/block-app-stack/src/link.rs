use std::sync::Arc;

use be_client::{ClientError, Clock, Connection};
use be_protocol::client;
use tokio::sync::{Mutex, OwnedMutexGuard};
use uuid::Uuid;

struct Held {
    socket: String,
    token: String,
    clock: Clock,
    link: Link,
}

#[derive(Clone)]
pub struct Link {
    pub connection: Arc<Connection>,
    pub account: Uuid,
    organization: Arc<Mutex<()>>,
}

impl Link {
    pub async fn open_organization(&self) -> OwnedMutexGuard<()> {
        Arc::clone(&self.organization).lock_owned().await
    }
}

static HELD: Mutex<Vec<Held>> = Mutex::const_new(Vec::new());

pub async fn connect(server_url: &str) -> Result<Arc<Connection>, ClientError> {
    open(server_url, &Clock::real()).await
}

async fn open(server_url: &str, clock: &Clock) -> Result<Arc<Connection>, ClientError> {
    let socket = be_client::socket_url(server_url);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let clock = clock.clone();
        home()
            .spawn(async move { Connection::connect(&socket, &clock).await })
            .await
            .map_err(|error| ClientError::Disconnected(error.to_string()))?
    }
    #[cfg(target_arch = "wasm32")]
    {
        Connection::connect(&socket, clock).await
    }
}

pub async fn signed_in(server_url: &str, token: &str, clock: &Clock) -> Result<Link, ClientError> {
    let socket = be_client::socket_url(server_url);
    let mut held = HELD.lock().await;
    held.retain(|held| !held.link.connection.is_closed());
    if let Some(held) = held
        .iter()
        .find(|held| held.socket == socket && held.token == token && held.clock.same(clock))
    {
        return Ok(held.link.clone());
    }
    let connection = open(server_url, clock).await?;
    let account = be_client::ask(client::account::authenticate(&connection, token.to_owned()))
        .await?
        .account;
    let link = Link {
        connection,
        account,
        organization: Arc::new(Mutex::new(())),
    };
    held.push(Held {
        socket,
        token: token.to_owned(),
        clock: clock.clone(),
        link: link.clone(),
    });
    Ok(link)
}

pub async fn adopt(server_url: &str, token: &str, account: Uuid, connection: Arc<Connection>) {
    let socket = be_client::socket_url(server_url);
    let mut held = HELD.lock().await;
    held.retain(|held| held.socket != socket || held.token != token);
    held.push(Held {
        socket,
        token: token.to_owned(),
        clock: Clock::real(),
        link: Link {
            connection,
            account,
            organization: Arc::new(Mutex::new(())),
        },
    });
}

pub async fn forget(server_url: &str, token: &str) {
    let socket = be_client::socket_url(server_url);
    HELD.lock()
        .await
        .retain(|held| held.socket != socket || held.token != token);
}

#[cfg(not(target_arch = "wasm32"))]
fn home() -> &'static tokio::runtime::Runtime {
    static HOME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("block-app-link")
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to start the connection runtime: {error}"))
    })
}
