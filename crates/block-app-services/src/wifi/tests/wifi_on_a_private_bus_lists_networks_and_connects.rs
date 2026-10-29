use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::mpsc::unbounded_channel;
use zbus::object_server::{InterfaceRef, SignalEmitter};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use super::super::network_manager::{PATH, SERVICE, SETTINGS_PATH, run_on};
use super::*;
use crate::dbus::test_bus::Bus;

const WAIT: Duration = Duration::from_secs(20);
const DEVICE_PATH: &str = "/org/freedesktop/NetworkManager/Devices/1";
const ACTIVE_PATH: &str = "/org/freedesktop/NetworkManager/ActiveConnection/1";
const HOME_POINT: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
const CAFE_POINT: &str = "/org/freedesktop/NetworkManager/AccessPoint/2";
const HIDDEN_POINT: &str = "/org/freedesktop/NetworkManager/AccessPoint/3";
const CONNECTIONS: u32 = 3;

fn path(path: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(path).expect("a path")
}

fn connection_path(index: u32) -> String {
    format!("{SETTINGS_PATH}/{index}")
}

fn none() -> OwnedObjectPath {
    path("/")
}

#[derive(Default)]
struct Store {
    listed: Vec<u32>,
    ssids: HashMap<u32, Vec<u8>>,
}

struct Heard(Mutex<SyncSender<String>>);

impl Heard {
    fn hear(&self, what: String) {
        let _ = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(what);
    }
}

struct FakeManager {
    enabled: bool,
    store: Arc<Mutex<Store>>,
    heard: Arc<Heard>,
}

type Settings = HashMap<String, HashMap<String, OwnedValue>>;

fn setting(settings: &Settings, group: &str, key: &str) -> String {
    let value = settings
        .get(group)
        .and_then(|group| group.get(key))
        .map(|value| &**value);
    let value = match value {
        Some(Value::Value(boxed)) => Some(&**boxed),
        value => value,
    };
    match value {
        Some(Value::Str(text)) => text.as_str().to_owned(),
        Some(Value::Array(bytes)) => bytes
            .inner()
            .iter()
            .filter_map(|byte| match byte {
                Value::U8(byte) => Some(char::from(*byte)),
                _ => None,
            })
            .collect(),
        _ => "-".to_owned(),
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl FakeManager {
    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.enabled
    }

    #[zbus(property)]
    fn set_wireless_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.heard.hear(format!("Enabled {enabled}"));
    }

    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path(DEVICE_PATH)]
    }

    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        device: OwnedObjectPath,
        specific: OwnedObjectPath,
    ) -> OwnedObjectPath {
        self.heard.hear(format!(
            "Activate {} {} {}",
            connection.as_str(),
            device.as_str(),
            specific.as_str()
        ));
        path(ACTIVE_PATH)
    }

    fn add_and_activate_connection(
        &self,
        settings: Settings,
        device: OwnedObjectPath,
        specific: OwnedObjectPath,
    ) -> (OwnedObjectPath, OwnedObjectPath) {
        let ssid = setting(&settings, "802-11-wireless", "ssid");
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        let index = (1..=CONNECTIONS)
            .find(|index| !store.listed.contains(index))
            .unwrap_or(CONNECTIONS);
        store.listed.push(index);
        store.ssids.insert(index, ssid.clone().into_bytes());
        self.heard.hear(format!(
            "Add {ssid} {} {} {} {}",
            setting(&settings, "802-11-wireless-security", "key-mgmt"),
            setting(&settings, "802-11-wireless-security", "psk"),
            device.as_str(),
            specific.as_str()
        ));
        (path(&connection_path(index)), path(ACTIVE_PATH))
    }
}

struct FakeDevice {
    state: u32,
    active: OwnedObjectPath,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl FakeDevice {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        self.state
    }

    #[zbus(property)]
    fn active_connection(&self) -> OwnedObjectPath {
        self.active.clone()
    }

    fn disconnect(&self) {}

    #[zbus(signal, name = "StateChanged")]
    async fn moved(
        emitter: &SignalEmitter<'_>,
        new: u32,
        old: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

struct FakeWireless;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl FakeWireless {
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        none()
    }

    fn get_all_access_points(&self) -> Vec<OwnedObjectPath> {
        [HOME_POINT, CAFE_POINT, HIDDEN_POINT].map(path).to_vec()
    }

    fn request_scan(&self, _options: HashMap<String, OwnedValue>) {}
}

struct FakePoint {
    ssid: &'static str,
    strength: u8,
    rsn: u32,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl FakePoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.as_bytes().to_vec()
    }

    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.strength
    }

    #[zbus(property)]
    fn flags(&self) -> u32 {
        u32::from(self.rsn != 0)
    }

    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.rsn
    }
}

struct FakeActive {
    connection: OwnedObjectPath,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl FakeActive {
    #[zbus(property)]
    fn connection(&self) -> OwnedObjectPath {
        self.connection.clone()
    }
}

struct FakeSettings {
    store: Arc<Mutex<Store>>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl FakeSettings {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        let store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        store
            .listed
            .iter()
            .map(|index| path(&connection_path(*index)))
            .collect()
    }
}

struct FakeConnection {
    index: u32,
    store: Arc<Mutex<Store>>,
    heard: Arc<Heard>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl FakeConnection {
    fn get_settings(&self) -> HashMap<String, HashMap<String, OwnedValue>> {
        let store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        let ssid = store.ssids.get(&self.index).cloned().unwrap_or_default();
        let owned = |value: Value<'_>| value.try_into_owned().expect("no file descriptors");
        HashMap::from([
            (
                "connection".to_owned(),
                HashMap::from([("type".to_owned(), owned(Value::from("802-11-wireless")))]),
            ),
            (
                "802-11-wireless".to_owned(),
                HashMap::from([("ssid".to_owned(), owned(Value::from(ssid)))]),
            ),
        ])
    }

    fn delete(&self) {
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        store.listed.retain(|index| *index != self.index);
        let ssid = store.ssids.remove(&self.index).unwrap_or_default();
        self.heard
            .hear(format!("Delete {}", String::from_utf8_lossy(&ssid)));
    }
}

async fn moved(device: &InterfaceRef<FakeDevice>, new: u32, old: u32) {
    device.get_mut().await.state = new;
    FakeDevice::moved(device.signal_emitter(), new, old, 0)
        .await
        .expect("the device signals");
}

fn until(published: &Receiver<WifiStatus>, what: &str, done: impl Fn(&WifiStatus) -> bool) {
    loop {
        let status = published
            .recv_timeout(WAIT)
            .unwrap_or_else(|_| panic!("the status shows {what}"));
        if done(&status) {
            return;
        }
    }
}

fn heard(heard: &Receiver<String>, what: &str) {
    let got = heard
        .recv_timeout(WAIT)
        .unwrap_or_else(|_| panic!("NetworkManager hears {what}"));
    assert_eq!(got, what);
}

#[test]
fn wifi_on_a_private_bus_lists_networks_and_connects() {
    let bus = Bus::start();
    let (published_sender, published) = std::sync::mpsc::channel();
    let (heard_sender, heard_by_manager) = sync_channel(16);
    let (requests, requested) = unbounded_channel();
    let ears = Arc::new(Heard(Mutex::new(heard_sender)));
    let store = Arc::new(Mutex::new(Store {
        listed: vec![1],
        ssids: HashMap::from([(1, b"Home".to_vec())]),
    }));
    zbus::block_on(async {
        let host = bus.connect().await;
        let manager = bus.connect().await;
        let server = manager.object_server();
        server
            .at(
                PATH,
                FakeManager {
                    enabled: true,
                    store: store.clone(),
                    heard: ears.clone(),
                },
            )
            .await
            .expect("the manager is served");
        server
            .at(
                DEVICE_PATH,
                FakeDevice {
                    state: model::DISCONNECTED,
                    active: none(),
                },
            )
            .await
            .expect("the device is served");
        server
            .at(DEVICE_PATH, FakeWireless)
            .await
            .expect("the device's Wi-Fi is served");
        for (at, ssid, strength, rsn) in [
            (HOME_POINT, "Home", 72, PSK),
            (CAFE_POINT, "Cafe", 41, 0),
            (HIDDEN_POINT, "", 90, PSK),
        ] {
            server
                .at(
                    at,
                    FakePoint {
                        ssid,
                        strength,
                        rsn,
                    },
                )
                .await
                .expect("the access point is served");
        }
        server
            .at(
                ACTIVE_PATH,
                FakeActive {
                    connection: path(&connection_path(1)),
                },
            )
            .await
            .expect("the active connection is served");
        server
            .at(
                SETTINGS_PATH,
                FakeSettings {
                    store: store.clone(),
                },
            )
            .await
            .expect("the settings are served");
        for index in 1..=CONNECTIONS {
            server
                .at(
                    connection_path(index),
                    FakeConnection {
                        index,
                        store: store.clone(),
                        heard: ears.clone(),
                    },
                )
                .await
                .expect("the connection is served");
        }
        manager
            .request_name(SERVICE)
            .await
            .expect("the fake owns NetworkManager's name");

        crate::dbus::spawn(run_on(host, requested, move |status| {
            let _ = published_sender.send(status);
        }));
        until(&published, "the networks in range", |status| {
            status.available
                && status.enabled
                && status.link == WifiLink::Disconnected
                && status.networks
                    == vec![
                        WifiNetwork {
                            ssid: "Home".to_owned(),
                            strength: 72,
                            security: WifiSecurity::Password,
                            saved: true,
                        },
                        WifiNetwork {
                            ssid: "Cafe".to_owned(),
                            strength: 41,
                            security: WifiSecurity::Open,
                            saved: false,
                        },
                    ]
        });

        let device = server
            .interface::<_, FakeDevice>(DEVICE_PATH)
            .await
            .expect("the device is served");

        let _ = requests.send(WifiRequest::Connect {
            ssid: "Home".to_owned(),
            password: None,
        });
        heard(
            &heard_by_manager,
            &format!("Activate {} {DEVICE_PATH} {HOME_POINT}", connection_path(1)),
        );
        until(&published, "the network being joined", |status| {
            status.link == WifiLink::Connecting("Home".to_owned())
        });
        device.get_mut().await.active = path(ACTIVE_PATH);
        moved(&device, model::PREPARE, model::DISCONNECTED).await;
        moved(&device, model::ACTIVATED, model::PREPARE).await;
        until(&published, "the network joined", |status| {
            status.link == WifiLink::Connected("Home".to_owned()) && status.failed.is_none()
        });

        let _ = requests.send(WifiRequest::Connect {
            ssid: "Cafe".to_owned(),
            password: None,
        });
        heard(
            &heard_by_manager,
            &format!("Add Cafe - - {DEVICE_PATH} {CAFE_POINT}"),
        );
        moved(&device, model::DISCONNECTED, model::ACTIVATED).await;
        moved(&device, model::PREPARE, model::DISCONNECTED).await;
        moved(&device, model::FAILED, model::PREPARE).await;
        heard(&heard_by_manager, "Delete Cafe");
        until(
            &published,
            "the network that could not be joined",
            |status| {
                status.failed.as_deref() == Some("Cafe")
                    && status
                        .networks
                        .iter()
                        .all(|network| network.ssid != "Cafe" || !network.saved)
            },
        );

        let _ = requests.send(WifiRequest::Connect {
            ssid: "Home".to_owned(),
            password: Some("correct horse".to_owned()),
        });
        heard(
            &heard_by_manager,
            &format!("Add Home wpa-psk correct horse {DEVICE_PATH} {HOME_POINT}"),
        );
        until(
            &published,
            "the network being joined while the device still shows the last failure",
            |status| {
                status.failed.is_none() && status.link == WifiLink::Connecting("Home".to_owned())
            },
        );
        server
            .interface::<_, FakeActive>(ACTIVE_PATH)
            .await
            .expect("the active connection is served")
            .get_mut()
            .await
            .connection = path(&connection_path(2));
        moved(&device, model::PREPARE, model::FAILED).await;
        moved(&device, model::ACTIVATED, model::PREPARE).await;
        heard(&heard_by_manager, "Delete Home");
        until(
            &published,
            "the network joined with its new password",
            |status| {
                status.failed.is_none() && status.link == WifiLink::Connected("Home".to_owned())
            },
        );
        assert_eq!(
            store.lock().unwrap_or_else(PoisonError::into_inner).listed,
            vec![2],
            "the new connection replaces the one saved before"
        );

        let _ = requests.send(WifiRequest::SetEnabled(false));
        heard(&heard_by_manager, "Enabled false");
        until(&published, "Wi-Fi turned off", |status| {
            !status.enabled && status.networks.is_empty()
        });
    });
}
