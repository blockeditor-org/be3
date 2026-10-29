use std::cell::RefCell;
use std::rc::Rc;

use block_plugin_api::{WifiLink, WifiNetwork, WifiSecurity};

use super::model::*;
use super::*;

mod a_connection_uses_what_is_saved_or_needs_a_password;
mod an_attempt_ends_once_the_device_connects_or_fails;
mod networks_are_listed_once_each_strongest_first;
mod requests_from_the_desktop_reach_the_backend_within_bounds;
mod wifi_on_a_private_bus_lists_networks_and_connects;

const PSK: u32 = 0x100;
const SAE: u32 = 0x400;
const EAP: u32 = 0x200;

fn point(ssid: &str, strength: u8, rsn: u32) -> AccessPoint {
    AccessPoint {
        ssid: ssid.as_bytes().to_vec(),
        strength,
        flags: u32::from(rsn != 0),
        wpa: 0,
        rsn,
    }
}

#[derive(Clone, Default)]
struct Recorder(Rc<RefCell<Vec<WifiRequest>>>);

impl Backend for Recorder {
    fn request(&self, request: WifiRequest) {
        self.0.borrow_mut().push(request);
    }
}

impl Recorder {
    fn sent(&self) -> Vec<WifiRequest> {
        self.0.borrow_mut().drain(..).collect()
    }
}
