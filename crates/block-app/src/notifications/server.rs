use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use be_wayland::programs::{DesktopEntry, Environment, IconThemes, load_icon};
use beui::Image;
use block_plugin_api::{
    HostImage, HostNotificationAction, IncomingNotification, NotificationRequest,
    NotificationSignal,
};
use image::imageops::FilterType;
use tokio::sync::mpsc::UnboundedReceiver;
use zbus::fdo::{self, RequestNameFlags, RequestNameReply};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

use super::hints;
use super::inbox::Ids;
use crate::host::WakingSender;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const ICON_PIXELS: u32 = 64;
const MAX_PICTURE: u32 = 2 * ICON_PIXELS;
const MAX_NAME: usize = 256;
const MAX_SUMMARY: usize = 1024;
const MAX_MARKUP: usize = 32 * 1024;
const MAX_ACTIONS: usize = 8;
const CAPABILITIES: [&str; 5] = [
    "actions",
    "body",
    "body-markup",
    "icon-static",
    "persistence",
];

struct Looks {
    environment: Environment,
    themes: IconThemes,
}

struct Server {
    requests: WakingSender<NotificationRequest>,
    ids: Arc<Mutex<Ids>>,
    looks: OnceLock<Looks>,
}

impl Server {
    fn looks(&self) -> &Looks {
        self.looks.get_or_init(|| {
            let environment = Environment::from_env();
            let themes = IconThemes::new(&environment);
            Looks {
                environment,
                themes,
            }
        })
    }

    fn icon(&self, name: &str) -> Option<Image> {
        let name = name.strip_prefix("file://").unwrap_or(name);
        if name.is_empty() {
            return None;
        }
        let path = match name.starts_with('/') {
            true => PathBuf::from(name),
            false => self.looks().themes.find(name, ICON_PIXELS)?,
        };
        load_icon(&path, ICON_PIXELS)
    }

    fn desktop_entry(&self, id: &str) -> Option<DesktopEntry> {
        let id = id.trim_end_matches(".desktop");
        if id.is_empty() || id.contains('/') {
            return None;
        }
        let looks = self.looks();
        looks.environment.data_dirs.iter().find_map(|dir| {
            let path = dir.join("applications").join(format!("{id}.desktop"));
            let text = std::fs::read_to_string(&path).ok()?;
            DesktopEntry::parse(
                id,
                &path.to_string_lossy(),
                &text,
                &looks.environment.locales,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn incoming(
        &self,
        app_name: &str,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[String],
        hints: &HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> IncomingNotification {
        let hints = hints::read(hints);
        let entry = hints
            .desktop_entry
            .as_deref()
            .and_then(|id| self.desktop_entry(id));
        let image = hints
            .image
            .or_else(|| hints.image_path.as_deref().and_then(|path| self.icon(path)))
            .or_else(|| self.icon(app_icon))
            .or_else(|| {
                let icon = entry.as_ref()?.icon.as_deref()?;
                self.icon(icon)
            });
        let app_name = match (app_name.is_empty(), &entry) {
            (true, Some(entry)) => entry.name.clone(),
            _ => app_name.to_owned(),
        };
        IncomingNotification {
            id: 0,
            app_name: clip(&app_name, MAX_NAME),
            summary: clip(summary, MAX_SUMMARY),
            body: clip(body, MAX_MARKUP),
            actions: actions
                .as_chunks::<2>()
                .0
                .iter()
                .take(MAX_ACTIONS)
                .map(|pair| HostNotificationAction {
                    key: clip(&pair[0], MAX_NAME),
                    label: clip(&pair[1], MAX_NAME),
                })
                .collect(),
            urgency: hints.urgency,
            image: image.map(picture),
            transient: hints.transient,
            resident: hints.resident,
            expire_timeout,
            received: unix_seconds(),
        }
    }
}

fn picture(image: Image) -> HostImage {
    let (width, height) = (image.width(), image.height());
    let rgba = image.pixels().to_vec();
    let fits = width.max(height) <= MAX_PICTURE;
    let shrunk = (!fits)
        .then(|| image::RgbaImage::from_raw(width, height, rgba.clone()))
        .flatten()
        .map(|full| {
            let scale = MAX_PICTURE as f32 / width.max(height) as f32;
            let side = |length: u32| ((length as f32 * scale).round() as u32).max(1);
            image::imageops::resize(&full, side(width), side(height), FilterType::Triangle)
        });
    match shrunk {
        Some(shrunk) => HostImage {
            width: shrunk.width(),
            height: shrunk.height(),
            rgba: shrunk.into_raw(),
        },
        None => HostImage {
            width,
            height,
            rgba,
        },
    }
}

fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> fdo::Result<u32> {
        let mut incoming = self.incoming(
            &app_name,
            &app_icon,
            &summary,
            &body,
            &actions,
            &hints,
            expire_timeout,
        );
        let mut ids = self.ids.lock().unwrap_or_else(PoisonError::into_inner);
        incoming.id = ids.assign(replaces_id);
        let id = incoming.id;
        self.requests
            .send(NotificationRequest::Notify(Box::new(incoming)))
            .map_err(|_| fdo::Error::Failed("the desktop has closed".to_owned()))?;
        Ok(id)
    }

    async fn close_notification(&self, id: u32) {
        let _ = self.requests.send(NotificationRequest::Close(id));
    }

    fn get_capabilities(&self) -> Vec<String> {
        CAPABILITIES.map(str::to_owned).to_vec()
    }

    #[zbus(out_args("name", "vendor", "version", "spec_version"))]
    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Block".to_owned(),
            "Block".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            "1.2".to_owned(),
        )
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

pub(crate) async fn run(
    requests: WakingSender<NotificationRequest>,
    mut signals: UnboundedReceiver<NotificationSignal>,
    ids: Arc<Mutex<Ids>>,
) {
    let Some(connection) = crate::dbus::session().await else {
        return;
    };
    let server = Server {
        requests,
        ids,
        looks: OnceLock::new(),
    };
    match connection.object_server().at(PATH, server).await {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("block-app: notifications are already served on this connection");
            return;
        }
        Err(error) => {
            eprintln!("block-app: notifications cannot be served: {error}");
            return;
        }
    }
    match connection
        .request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
        Ok(_) => {
            eprintln!("block-app: another notification server is running, so this one is not");
            let _ = connection.object_server().remove::<Server, _>(PATH).await;
            return;
        }
        Err(error) => {
            eprintln!("block-app: notifications cannot be served: {error}");
            let _ = connection.object_server().remove::<Server, _>(PATH).await;
            return;
        }
    }
    let emitter = match SignalEmitter::new(&connection, PATH) {
        Ok(emitter) => emitter,
        Err(error) => {
            eprintln!("block-app: notification signals cannot be sent: {error}");
            return;
        }
    };
    while let Some(signal) = signals.recv().await {
        let sent = match &signal {
            NotificationSignal::Closed(id, reason) => {
                Server::notification_closed(&emitter, *id, *reason as u32).await
            }
            NotificationSignal::ActionInvoked(id, key) => {
                Server::action_invoked(&emitter, *id, key).await
            }
        };
        if let Err(error) = sent {
            eprintln!("block-app: a notification signal was not sent: {error}");
        }
    }
    let _ = connection.object_server().remove::<Server, _>(PATH).await;
    let _ = connection.release_name(NAME).await;
}
