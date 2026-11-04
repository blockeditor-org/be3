mod model;
mod network_manager;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use be_host_base::host::{self, Mailbox};

use be_sdk_api::{WifiRequest, WifiStatus, WifiWatch};

pub trait Backend {
    fn request(&self, request: WifiRequest);
}

pub struct Wifi {
    backend: Rc<dyn Backend>,
    _reports: Mailbox,
}

impl Wifi {
    pub fn start() -> Self {
        be_host_base::watches::publish::<WifiWatch>(&WifiStatus::default());
        let (reports, post) = host::mailbox(|status: WifiStatus| {
            be_host_base::watches::publish::<WifiWatch>(&status)
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
            let keyed = password
                .as_deref()
                .is_none_or(|password| model::passphrase(password) || model::wep_key(password));
            (named(&ssid) && keyed).then_some(WifiRequest::Connect { ssid, password })
        }
        WifiRequest::JoinHidden {
            ssid,
            security,
            password,
        } => {
            let keyed = model::Key::hidden(security, password.clone()).is_some();
            (named(&ssid) && ssid.len() <= LONGEST_SSID && keyed).then_some(
                WifiRequest::JoinHidden {
                    ssid,
                    security,
                    password,
                },
            )
        }
        request => Some(request),
    }
}

const LONGEST_SSID: usize = 32;

fn named(ssid: &str) -> bool {
    !ssid.is_empty() && !ssid.contains('\0')
}
