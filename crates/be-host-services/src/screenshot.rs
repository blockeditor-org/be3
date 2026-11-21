//! The screenshot portal's backend, `org.freedesktop.impl.portal.Screenshot`.
//! xdg-desktop-portal forwards `org.freedesktop.portal.Screenshot` here under the
//! `be-host` desktop (the `.portal` file and `be-host-portals.conf` the login
//! session installs); the picture is the host's to take, on its own thread.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use be_host_base::host::{self, Mailbox};
use beui_types::Image;
use tokio::sync::oneshot;
use zbus::Connection;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::dbus::Bus;

/// The name the `.portal` file the login session installs names.
pub const NAME: &str = "org.freedesktop.impl.portal.desktop.behost";
const PATH: &str = "/org/freedesktop/portal/desktop";
const SUCCESS: u32 = 0;
const FAILED: u32 = 2;

pub type Answer = oneshot::Sender<Option<Image>>;
pub type Ask = Arc<dyn Fn(Answer) + Send + Sync>;

pub struct Screenshots {
    _requests: Mailbox,
}

impl Screenshots {
    /// Serves the portal backend. `take` runs on this thread for each request and
    /// answers, then or later on this thread, with the whole desktop, or `None`
    /// when it cannot.
    pub fn start(take: impl Fn(Box<dyn FnOnce(Option<Image>)>) + 'static) -> Self {
        let (requests, post) = host::mailbox(move |answer: Answer| {
            take(Box::new(move |picture| {
                let _ = answer.send(picture);
            }));
        });
        let ask: Ask = Arc::new(move |answer| post.send(answer));
        let directory = directory();
        crate::dbus::spawn(async move {
            serve(&crate::dbus::SESSION, ask, directory).await;
        });
        Self {
            _requests: requests,
        }
    }
}

/// Where pictures are left for the program that asked, which reads them and
/// usually deletes them: a directory of the session's own, gone at log out.
fn directory() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|dir| !dir.is_empty())
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("be-host")
        .join("screenshots")
}

struct Portal {
    ask: Ask,
    directory: PathBuf,
}

impl Portal {
    async fn take(&self) -> Result<PathBuf, String> {
        let (answer, answered) = oneshot::channel();
        (self.ask)(answer);
        let picture = answered
            .await
            .ok()
            .flatten()
            .ok_or("the desktop has no picture to give")?;
        let directory = self.directory.clone();
        let (saved, heard) = oneshot::channel();
        std::thread::Builder::new()
            .name("screenshot".to_owned())
            .spawn(move || {
                let _ = saved.send(save(&directory, &picture));
            })
            .map_err(|error| error.to_string())?;
        heard
            .await
            .map_err(|_| "the picture was not saved".to_owned())?
    }
}

fn save(directory: &Path, picture: &Image) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let path = directory.join(format!("Screenshot-{millis}-{}.png", uuid::Uuid::new_v4()));
    image::save_buffer(
        &path,
        picture.pixels(),
        picture.width(),
        picture.height(),
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|error| error.to_string())?;
    Ok(path)
}

/// A `file://` URI, escaping every byte a path segment cannot hold.
pub fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = "file://".to_owned();
    for &byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(char::from(byte));
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Screenshot")]
impl Portal {
    /// Takes the whole desktop at once: `interactive` asks for a dialog to choose
    /// what to take, which this backend does not have.
    async fn screenshot(
        &self,
        _handle: OwnedObjectPath,
        _app_id: String,
        _parent_window: String,
        _options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let mut results = HashMap::new();
        match self.take().await {
            Ok(path) => {
                let uri = Value::from(file_uri(&path));
                if let Ok(uri) = uri.try_to_owned() {
                    results.insert("uri".to_owned(), uri);
                }
                (SUCCESS, results)
            }
            Err(error) => {
                eprintln!("be-host: no screenshot was taken: {error}");
                (FAILED, results)
            }
        }
    }

    /// Version 1, so xdg-desktop-portal asks no access dialog first: it calls this
    /// backend for every program, which may take screenshots anyway.
    #[zbus(property)]
    fn version(&self) -> u32 {
        1
    }
}

pub async fn serve(bus: &Bus, ask: Ask, directory: PathBuf) {
    loop {
        let connection = bus.connected().await;
        let serving = run_on(connection.clone(), Arc::clone(&ask), directory.clone());
        if crate::dbus::until_closed(&connection, serving)
            .await
            .is_break()
        {
            return;
        }
    }
}

async fn run_on(connection: Connection, ask: Ask, directory: PathBuf) -> ControlFlow<()> {
    let portal = Portal { ask, directory };
    match connection.object_server().at(PATH, portal).await {
        Ok(true) => {}
        Ok(false) => return ControlFlow::Continue(()),
        Err(error) => {
            eprintln!("be-host: the screenshot portal cannot be served: {error}");
            return ControlFlow::Continue(());
        }
    }
    match connection
        .request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
        Ok(_) => {
            eprintln!("be-host: another be-host serves screenshots, so this one does not");
            let _ = connection.object_server().remove::<Portal, _>(PATH).await;
            return ControlFlow::Continue(());
        }
        Err(error) => {
            eprintln!("be-host: the screenshot portal cannot be served: {error}");
            let _ = connection.object_server().remove::<Portal, _>(PATH).await;
            return ControlFlow::Continue(());
        }
    }
    std::future::pending().await
}
