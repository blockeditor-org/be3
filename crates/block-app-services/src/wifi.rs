mod model;
mod network_manager;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use block_app_base::host::{self, Mailbox};

use block_plugin_api::{WifiRequest, WifiStatus, WifiWatch};

const PASSPHRASE_BYTES: std::ops::RangeInclusive<usize> = 8..=63;
const HEX_KEY_LENGTH: usize = 64;

pub trait Backend {
    fn request(&self, request: WifiRequest);
}

pub struct Wifi {
    backend: Rc<dyn Backend>,
    _reports: Mailbox,
}

impl Wifi {
    pub fn start() -> Self {
        block_app_base::watches::publish::<WifiWatch>(&WifiStatus::default());
        let (reports, post) = host::mailbox(|status: WifiStatus| {
            block_app_base::watches::publish::<WifiWatch>(&status)
        });
        let backend = network_manager::NetworkManager::start(post);
        Self::with(Rc::new(backend), reports)
    }

    pub fn with(backend: Rc<dyn Backend>, reports: Mailbox) -> Self {
        Self {
            backend,
            _reports: reports,
        }
    }

    pub fn request(&self, request: WifiRequest) {
        if let Some(request) = checked(request) {
            self.backend.request(request);
        }
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
