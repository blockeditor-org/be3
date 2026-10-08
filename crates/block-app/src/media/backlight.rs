use std::path::Path;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::Proxy;

use super::{Backlight, MediaEvent};
use crate::host::WakingSender;

const DEVICES: &str = "/sys/class/backlight";
const DESTINATION: &str = "org.freedesktop.login1";
const SESSION: &str = "/org/freedesktop/login1/session/auto";
const INTERFACE: &str = "org.freedesktop.login1.Session";

pub(crate) struct LogindBacklight {
    steps: UnboundedSender<f32>,
}

impl LogindBacklight {
    pub(crate) fn start(events: WakingSender<MediaEvent>) -> Self {
        let (steps, stepped) = unbounded_channel();
        crate::dbus::spawn(run(stepped, events));
        Self { steps }
    }
}

impl Backlight for LogindBacklight {
    fn step(&self, by: f32) {
        let _ = self.steps.send(by);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Device {
    pub(super) name: String,
    pub(super) brightness: u32,
    pub(super) max: u32,
}

pub(super) fn find(devices: &Path) -> Option<Device> {
    let mut found: Vec<(usize, Device)> = std::fs::read_dir(devices)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let read = |name: &str| std::fs::read_to_string(path.join(name)).ok();
            let rank = match read("type")?.trim() {
                "firmware" => 0,
                "platform" => 1,
                "raw" => 2,
                _ => 3,
            };
            let max = read("max_brightness")?.trim().parse().ok()?;
            let brightness = read("brightness")?.trim().parse().ok()?;
            let name = entry.file_name().into_string().ok()?;
            (max > 0).then_some((
                rank,
                Device {
                    name,
                    brightness,
                    max,
                },
            ))
        })
        .collect();
    found
        .sort_by(|(rank, device), (other, next)| rank.cmp(other).then(device.name.cmp(&next.name)));
    found.into_iter().next().map(|(_, device)| device)
}

pub(super) fn stepped(brightness: u32, max: u32, by: f32) -> u32 {
    let step = ((max as f32 * by.abs()).round() as u32).max(1);
    match by > 0.0 {
        true => brightness.saturating_add(step).min(max),
        false => brightness.saturating_sub(step).max(1.min(max)),
    }
}

async fn run(mut steps: UnboundedReceiver<f32>, events: WakingSender<MediaEvent>) {
    let mut session = None;
    while let Some(by) = steps.recv().await {
        let Some(device) = find(Path::new(DEVICES)) else {
            eprintln!("block-app: there is no backlight in {DEVICES} to brighten or dim");
            continue;
        };
        let target = stepped(device.brightness, device.max, by);
        if target != device.brightness {
            if session.is_none() {
                session = connect().await;
            }
            let Some(proxy) = &session else {
                continue;
            };
            let set = proxy
                .call::<_, _, ()>(
                    "SetBrightness",
                    &("backlight", device.name.as_str(), target),
                )
                .await;
            if let Err(error) = set {
                eprintln!("block-app: logind did not set the brightness: {error}");
                continue;
            }
        }
        let _ = events.send(MediaEvent::Brightness(target as f32 / device.max as f32));
    }
}

async fn connect() -> Option<Proxy<'static>> {
    let connection = crate::dbus::system().await?;
    match Proxy::new(&connection, DESTINATION, SESSION, INTERFACE).await {
        Ok(proxy) => Some(proxy),
        Err(error) => {
            eprintln!("block-app: logind's session is not reachable: {error}");
            None
        }
    }
}
