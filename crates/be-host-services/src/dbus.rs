#[cfg(test)]
pub mod test_bus;
#[cfg(test)]
mod tests;

use std::future::{Future, pending};
use std::sync::Once;

use async_executor::Executor;
use async_lock::OnceCell;
use zbus::Connection;
use zbus::connection::Builder;

static EXECUTOR: Executor<'static> = Executor::new();
static STARTED: Once = Once::new();
static SYSTEM: OnceCell<Option<Connection>> = OnceCell::new();
static SESSION: OnceCell<Option<Connection>> = OnceCell::new();

pub fn spawn(task: impl Future<Output = ()> + Send + 'static) {
    STARTED.call_once(|| {
        let started = std::thread::Builder::new()
            .name("dbus".to_owned())
            .spawn(|| zbus::block_on(EXECUTOR.run(pending::<()>())));
        if let Err(error) = started {
            eprintln!("be-host: the D-Bus thread did not start: {error}");
        }
    });
    EXECUTOR
        .spawn(async move {
            if let Err(panic) = be_host_base::panic_guard::contain_task(task).await {
                be_host_base::notices::report(format!("A part of the desktop stopped: {panic}"));
            }
        })
        .detach();
}

pub async fn system() -> Option<Connection> {
    SYSTEM
        .get_or_init(|| connect("system", async { build(Builder::system()?).await }))
        .await
        .clone()
}

pub async fn session() -> Option<Connection> {
    SESSION
        .get_or_init(|| connect("session", async { build(Builder::session()?).await }))
        .await
        .clone()
}

pub async fn build(builder: Builder<'_>) -> zbus::Result<Connection> {
    builder
        .serve_at("/", zbus::fdo::ObjectManager)?
        .build()
        .await
}

async fn connect(
    bus: &str,
    connecting: impl Future<Output = zbus::Result<Connection>>,
) -> Option<Connection> {
    match connecting.await {
        Ok(connection) => Some(connection),
        Err(error) => {
            eprintln!("be-host: the D-Bus {bus} bus is not reachable: {error}");
            None
        }
    }
}
