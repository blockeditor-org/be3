use std::path::Path;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::{Backlight, MediaEvent};
use be_host_base::host::Post;

const DEVICES: &str = "/sys/class/backlight";
const DESTINATION: &str = "org.freedesktop.login1";
const SESSION: &str = "/org/freedesktop/login1/session/auto";
const INTERFACE: &str = "org.freedesktop.login1.Session";

pub struct LogindBacklight {
    steps: UnboundedSender<f32>,
}

impl LogindBacklight {
    pub fn start(events: Post<MediaEvent>) -> Self {
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

async fn run(mut steps: UnboundedReceiver<f32>, events: Post<MediaEvent>) {
    let mut told = false;
    if let Some(device) = find(Path::new(DEVICES)) {
        events.send(MediaEvent::Brightness(
            device.brightness as f32 / device.max as f32,
        ));
    }
    while let Some(by) = steps.recv().await {
        let Some(device) = find(Path::new(DEVICES)) else {
            if !std::mem::replace(&mut told, true) {
                eprintln!("be-host: there is no backlight in {DEVICES} to brighten or dim");
            }
            continue;
        };
        told = false;
        let target = stepped(device.brightness, device.max, by);
        if target != device.brightness {
            let Some(connection) = crate::dbus::SYSTEM.connection().await else {
                continue;
            };
            let set = connection
                .call_method(
                    Some(DESTINATION),
                    SESSION,
                    Some(INTERFACE),
                    "SetBrightness",
                    &("backlight", device.name.as_str(), target),
                )
                .await;
            if let Err(error) = set {
                eprintln!("be-host: logind did not set the brightness: {error}");
                continue;
            }
        }
        events.send(MediaEvent::Brightness(target as f32 / device.max as f32));
    }
}
