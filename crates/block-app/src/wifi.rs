mod model;
mod network_manager;
#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::mpsc::Receiver;

use block_plugin_api::{WifiRequest, WifiStatus};

const PASSPHRASE_BYTES: std::ops::RangeInclusive<usize> = 8..=63;
const HEX_KEY_LENGTH: usize = 64;

pub(crate) trait Backend {
    fn request(&self, request: WifiRequest);
}

pub(crate) struct Wifi {
    backend: Rc<dyn Backend>,
    reports: Receiver<WifiStatus>,
    status: WifiStatus,
}

impl Wifi {
    pub(crate) fn start() -> Self {
        let (sender, reports) = block_app_base::host::waking_channel();
        let backend = network_manager::NetworkManager::start(sender);
        Self::with(Rc::new(backend), reports)
    }

    pub(crate) fn with(backend: Rc<dyn Backend>, reports: Receiver<WifiStatus>) -> Self {
        Self {
            backend,
            reports,
            status: WifiStatus::default(),
        }
    }

    pub(crate) fn request(&self, request: WifiRequest) {
        if let Some(request) = checked(request) {
            self.backend.request(request);
        }
    }

    pub(crate) fn frame(&mut self) -> Option<WifiStatus> {
        let latest = self.reports.try_iter().last()?;
        (latest != self.status).then(|| {
            self.status = latest.clone();
            latest
        })
    }
}

fn checked(request: WifiRequest) -> Option<WifiRequest> {
    match request {
        WifiRequest::Connect { ssid, password } => {
            let named = !ssid.is_empty() && !ssid.contains('\0');
            let keyed = password.as_deref().is_none_or(passphrase);
            (named && keyed).then_some(WifiRequest::Connect { ssid, password })
        }
        request => Some(request),
    }
}

fn passphrase(password: &str) -> bool {
    let printable = password.chars().all(|letter| !letter.is_control());
    let hex = password.len() == HEX_KEY_LENGTH
        && password.chars().all(|letter| letter.is_ascii_hexdigit());
    printable && (PASSPHRASE_BYTES.contains(&password.len()) || hex)
}
