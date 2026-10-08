use std::future::{Future, pending};
use std::sync::Once;

use async_executor::Executor;
use async_lock::OnceCell;
use zbus::Connection;

static EXECUTOR: Executor<'static> = Executor::new();
static STARTED: Once = Once::new();
static SYSTEM: OnceCell<Option<Connection>> = OnceCell::new();

pub(crate) fn spawn(task: impl Future<Output = ()> + Send + 'static) {
    STARTED.call_once(|| {
        let started = std::thread::Builder::new()
            .name("dbus".to_owned())
            .spawn(|| zbus::block_on(EXECUTOR.run(pending::<()>())));
        if let Err(error) = started {
            eprintln!("block-app: the D-Bus thread did not start: {error}");
        }
    });
    EXECUTOR.spawn(task).detach();
}

pub(crate) async fn system() -> Option<Connection> {
    SYSTEM
        .get_or_init(|| connect("system", Connection::system()))
        .await
        .clone()
}

async fn connect(
    bus: &str,
    connecting: impl Future<Output = zbus::Result<Connection>>,
) -> Option<Connection> {
    match connecting.await {
        Ok(connection) => Some(connection),
        Err(error) => {
            eprintln!("block-app: the D-Bus {bus} bus is not reachable: {error}");
            None
        }
    }
}
