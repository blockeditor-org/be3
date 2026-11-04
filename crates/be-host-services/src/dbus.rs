#[cfg(test)]
pub mod test_bus;
#[cfg(test)]
mod tests;

use std::future::{Future, pending};
use std::ops::ControlFlow;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_executor::Executor;
use async_io::{Async, Timer};
use async_lock::Mutex;
use futures_util::future::{Either, select};
use futures_util::pin_mut;
use rustix::fs::inotify;
use zbus::Connection;
use zbus::connection::Builder;

pub const METHOD_TIMEOUT: Duration = Duration::from_secs(10);
const FIRST_RETRY: Duration = Duration::from_secs(1);
const LAST_RETRY: Duration = Duration::from_secs(30);
const SYSTEM_SOCKET: &str = "unix:path=/var/run/dbus/system_bus_socket";

static EXECUTOR: Executor<'static> = Executor::new();
static STARTED: Once = Once::new();

pub static SYSTEM: Bus = Bus::new(Source::System);
pub static SESSION: Bus = Bus::new(Source::Session);

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

/// Builds every connection, the tests' private bus included.
///
/// The method timeout makes a peer that never answers fail its call instead of holding the task.
/// `ObjectManager` is served while connecting because zbus starts a connection's object server
/// only when the first object is served and drops calls that arrive before it listens, which
/// would lose a client's first call to a name served right after connecting.
pub async fn build(builder: Builder<'_>) -> zbus::Result<Connection> {
    builder
        .method_timeout(METHOD_TIMEOUT)
        .serve_at("/", zbus::fdo::ObjectManager)?
        .build()
        .await
}

/// Runs a service until it ends or the connection closes. A service returns `Break` only when
/// the desktop no longer wants it, and `Continue` when it cannot work on this connection (another
/// owner holds its name), which waits for the next connection.
pub async fn until_closed(
    connection: &Connection,
    service: impl Future<Output = ControlFlow<()>>,
) -> ControlFlow<()> {
    let closed = connection.closed();
    pin_mut!(service, closed);
    if let Either::Left((ControlFlow::Break(()), _)) = select(service, closed).await {
        return ControlFlow::Break(());
    }
    connection.closed().await;
    ControlFlow::Continue(())
}

pub enum Source {
    System,
    Session,
    Address(String),
}

pub struct Bus {
    source: Source,
    current: Mutex<Option<Connection>>,
    unreachable: AtomicBool,
}

impl Bus {
    pub const fn new(source: Source) -> Self {
        Self {
            source,
            current: Mutex::new(None),
            unreachable: AtomicBool::new(false),
        }
    }

    fn name(&self) -> &str {
        match &self.source {
            Source::System => "system",
            Source::Session => "session",
            Source::Address(address) => address,
        }
    }

    pub async fn connection(&self) -> Option<Connection> {
        let mut current = self.current.lock().await;
        if let Some(open) = &*current {
            if !open.is_closed() {
                return Some(open.clone());
            }
            eprintln!("be-host: the D-Bus {} bus closed", self.name());
            *current = None;
        }
        let connecting = match &self.source {
            Source::System => Builder::system(),
            Source::Session => Builder::session(),
            Source::Address(address) => Builder::address(address.as_str()),
        };
        let connected = match connecting {
            Ok(builder) => build(builder).await,
            Err(error) => Err(error),
        };
        match connected {
            Ok(connection) => {
                if self.unreachable.swap(false, Ordering::Relaxed) {
                    eprintln!("be-host: the D-Bus {} bus is reachable again", self.name());
                }
                *current = Some(connection.clone());
                Some(connection)
            }
            Err(error) => {
                if !self.unreachable.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "be-host: the D-Bus {} bus is not reachable, so it is tried again \
                         once it appears: {error}",
                        self.name()
                    );
                }
                None
            }
        }
    }

    pub async fn connected(&self) -> Connection {
        let mut wait = FIRST_RETRY;
        loop {
            let appearing = self.socket().and_then(|socket| Appearing::watch(&socket));
            if let Some(connection) = self.connection().await {
                return connection;
            }
            let timer = Timer::after(wait);
            match &appearing {
                Some(appearing) => {
                    let appeared = appearing.changed();
                    pin_mut!(appeared);
                    select(appeared, timer).await;
                }
                None => {
                    timer.await;
                }
            }
            wait = (wait * 2).min(LAST_RETRY);
        }
    }

    fn socket(&self) -> Option<PathBuf> {
        let address = match &self.source {
            Source::System => std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
                .unwrap_or_else(|_| SYSTEM_SOCKET.to_owned()),
            Source::Session => std::env::var("DBUS_SESSION_BUS_ADDRESS").ok().or_else(|| {
                let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
                Some(format!("unix:path={runtime}/bus"))
            })?,
            Source::Address(address) => address.clone(),
        };
        unix_path(&address)
    }
}

pub fn unix_path(address: &str) -> Option<PathBuf> {
    address.split(';').find_map(|one| {
        let options = one.strip_prefix("unix:")?;
        let path = options
            .split(',')
            .find_map(|option| option.strip_prefix("path="))?;
        Some(PathBuf::from(unescape(path)?))
    })
}

fn unescape(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&first, after)) = rest.split_first() {
        match first {
            b'%' => {
                let hex = std::str::from_utf8(after.get(..2)?).ok()?;
                bytes.push(u8::from_str_radix(hex, 16).ok()?);
                rest = &after[2..];
            }
            byte => {
                bytes.push(byte);
                rest = after;
            }
        }
    }
    String::from_utf8(bytes).ok()
}

struct Appearing {
    events: Async<OwnedFd>,
}

impl Appearing {
    fn watch(socket: &Path) -> Option<Self> {
        let directory = socket.parent()?;
        let events =
            inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK).ok()?;
        let flags = inotify::WatchFlags::CREATE
            | inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::ATTRIB;
        inotify::add_watch(&events, directory, flags).ok()?;
        Some(Self {
            events: Async::new(events).ok()?,
        })
    }

    async fn changed(&self) {
        let _ = self.events.readable().await;
    }
}
