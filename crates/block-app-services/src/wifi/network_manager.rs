use std::collections::HashMap;

use futures_util::future::{Either, select};
use futures_util::{FutureExt, Stream, StreamExt, pin_mut};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::message::Type;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, Message, MessageStream};

use block_plugin_api::{WifiLink, WifiRequest, WifiStatus};

use super::Backend;
use super::model::{self, AccessPoint, Attempts, Outcome, Plan};
use block_app_base::host::WakingSender;

pub const SERVICE: &str = "org.freedesktop.NetworkManager";
pub const PATH: &str = "/org/freedesktop/NetworkManager";
pub const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const MANAGER: &str = SERVICE;
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const BUS: &str = "org.freedesktop.DBus";
const WIRELESS_TYPE: &str = "802-11-wireless";
const SECURITY_TYPE: &str = "802-11-wireless-security";
const DEVICE_TYPE_WIFI: u32 = 2;
const NO_OBJECT: &str = "/";

pub struct NetworkManager {
    requests: UnboundedSender<WifiRequest>,
}

impl NetworkManager {
    pub fn start(reports: WakingSender<WifiStatus>) -> Self {
        let (requests, requested) = unbounded_channel();
        crate::dbus::spawn(async move {
            let Some(connection) = crate::dbus::system().await else {
                return;
            };
            run_on(connection, requested, move |status| {
                let _ = reports.send(status);
            })
            .await;
        });
        Self { requests }
    }
}

impl Backend for NetworkManager {
    fn request(&self, request: WifiRequest) {
        let _ = self.requests.send(request);
    }
}

type Properties = HashMap<String, OwnedValue>;
type Settings = HashMap<String, HashMap<String, OwnedValue>>;

fn inner<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(boxed) => inner(boxed),
        value => value,
    }
}

fn number(properties: &Properties, name: &str) -> u32 {
    match properties.get(name).map(|value| inner(value)) {
        Some(Value::U32(number)) => *number,
        Some(Value::U8(number)) => u32::from(*number),
        _ => 0,
    }
}

fn flag(properties: &Properties, name: &str) -> bool {
    matches!(
        properties.get(name).map(|value| inner(value)),
        Some(Value::Bool(true))
    )
}

fn object(properties: &Properties, name: &str) -> Option<OwnedObjectPath> {
    match properties.get(name).map(|value| inner(value)) {
        Some(Value::ObjectPath(path)) if path.as_str() != NO_OBJECT => {
            OwnedObjectPath::try_from(path.as_str()).ok()
        }
        _ => None,
    }
}

fn text(properties: &HashMap<String, OwnedValue>, name: &str) -> Option<String> {
    match properties.get(name).map(|value| inner(value)) {
        Some(Value::Str(text)) => Some(text.as_str().to_owned()),
        _ => None,
    }
}

fn bytes(properties: &HashMap<String, OwnedValue>, name: &str) -> Vec<u8> {
    let Some(Value::Array(array)) = properties.get(name).map(|value| inner(value)) else {
        return Vec::new();
    };
    array
        .inner()
        .iter()
        .filter_map(|byte| match byte {
            Value::U8(byte) => Some(*byte),
            _ => None,
        })
        .collect()
}

fn access_point(properties: &Properties) -> AccessPoint {
    AccessPoint {
        ssid: bytes(properties, "Ssid"),
        strength: u8::try_from(number(properties, "Strength")).unwrap_or(u8::MAX),
        flags: number(properties, "Flags"),
        wpa: number(properties, "WpaFlags"),
        rsn: number(properties, "RsnFlags"),
    }
}

fn saved_ssid(settings: &Settings) -> Option<Vec<u8>> {
    let kind = settings
        .get("connection")
        .and_then(|connection| text(connection, "type"));
    if kind.as_deref() != Some(WIRELESS_TYPE) {
        return None;
    }
    let ssid = bytes(settings.get(WIRELESS_TYPE)?, "ssid");
    (!ssid.is_empty()).then_some(ssid)
}

fn added_settings<'a>(
    ssid: Vec<u8>,
    key_management: Option<&'a str>,
    password: Option<String>,
) -> HashMap<&'static str, HashMap<&'static str, Value<'a>>> {
    let mut settings = HashMap::new();
    settings.insert(WIRELESS_TYPE, HashMap::from([("ssid", Value::from(ssid))]));
    if let Some(key_management) = key_management {
        let mut security = HashMap::from([("key-mgmt", Value::from(key_management))]);
        if let Some(password) = password {
            security.insert("psk", Value::from(password));
        }
        settings.insert(SECURITY_TYPE, security);
    }
    settings
}

#[derive(Default)]
struct Saved {
    paths: Vec<OwnedObjectPath>,
    ssids: Vec<Vec<u8>>,
}

struct Watcher {
    connection: Connection,
    saved: Saved,
    saved_stale: bool,
    device: Option<OwnedObjectPath>,
    paths: Vec<OwnedObjectPath>,
    points: Vec<AccessPoint>,
    attempts: Attempts,
    added: Option<OwnedObjectPath>,
    replaces: Vec<OwnedObjectPath>,
    reported: Option<WifiStatus>,
    unreadable: bool,
}

pub async fn run_on(
    connection: Connection,
    mut requested: UnboundedReceiver<WifiRequest>,
    mut report: impl FnMut(WifiStatus) + Send + 'static,
) {
    let mut signals = match watch(&connection).await {
        Ok(signals) => Some(signals),
        Err(error) => {
            eprintln!("block-app: NetworkManager cannot be watched: {error}");
            None
        }
    };
    let mut watcher = Watcher {
        connection,
        saved: Saved::default(),
        saved_stale: true,
        device: None,
        paths: Vec::new(),
        points: Vec::new(),
        attempts: Attempts::default(),
        added: None,
        replaces: Vec::new(),
        reported: None,
        unreadable: false,
    };
    watcher.refresh(&mut report).await;
    loop {
        let next = {
            let request = requested.recv();
            let signal = async {
                match &mut signals {
                    Some(signals) => signals.next().await,
                    None => std::future::pending().await,
                }
            };
            pin_mut!(request, signal);
            match select(request, signal).await {
                Either::Left((request, _)) => Either::Left(request),
                Either::Right((signal, _)) => Either::Right(signal),
            }
        };
        match next {
            Either::Left(Some(request)) => watcher.request(request).await,
            Either::Left(None) => return,
            Either::Right(Some(Ok(message))) => {
                watcher.observe(&message).await;
                if let Some(signals) = &mut signals {
                    while let Some(Some(Ok(message))) = signals.next().now_or_never() {
                        watcher.observe(&message).await;
                    }
                }
            }
            Either::Right(Some(Err(error))) => {
                eprintln!("block-app: a NetworkManager signal could not be read: {error}");
            }
            Either::Right(None) => signals = None,
        }
        watcher.refresh(&mut report).await;
    }
}

async fn watch(
    connection: &Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Message>> + Unpin + use<>> {
    let changes = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(SERVICE)?
        .build();
    let owners = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(BUS)?
        .interface(BUS)?
        .member("NameOwnerChanged")?
        .arg(0, SERVICE)?
        .build();
    Ok(futures_util::stream::select(
        MessageStream::for_match_rule(changes, connection, None).await?,
        MessageStream::for_match_rule(owners, connection, None).await?,
    ))
}

impl Watcher {
    async fn call<R>(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        body: &(impl serde::Serialize + zbus::zvariant::DynamicType),
    ) -> zbus::Result<R>
    where
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        self.connection
            .call_method(Some(SERVICE), path, Some(interface), method, body)
            .await?
            .body()
            .deserialize()
    }

    async fn properties(&self, path: &str, interface: &str) -> zbus::Result<Properties> {
        self.call(path, PROPERTIES, "GetAll", &(interface,)).await
    }

    async fn observe(&mut self, message: &Message) {
        let header = message.header();
        let interface = header.interface().map(|interface| interface.as_str());
        let member = header.member().map(|member| member.as_str());
        match (interface, member) {
            (Some(BUS), Some("NameOwnerChanged")) => {
                self.saved_stale = true;
                self.attempts.cancel();
                self.added = None;
                self.replaces.clear();
            }
            (Some(SETTINGS | CONNECTION), _) => self.saved_stale = true,
            (Some(DEVICE), Some("StateChanged")) => {
                let ours = header
                    .path()
                    .zip(self.device.as_ref())
                    .is_some_and(|(path, device)| path.as_str() == device.as_str());
                if let (true, Ok((state, _, _))) =
                    (ours, message.body().deserialize::<(u32, u32, u32)>())
                {
                    self.settle(state, None).await;
                }
            }
            _ => {}
        }
    }

    async fn settle(&mut self, state: u32, active: Option<&str>) {
        match self.attempts.state(state, active) {
            Some(Outcome::Connected) => {
                self.added = None;
                for path in std::mem::take(&mut self.replaces) {
                    self.forget(&path).await;
                }
            }
            Some(Outcome::Failed) => {
                self.replaces.clear();
                if let Some(path) = self.added.take() {
                    self.forget(&path).await;
                }
            }
            None => {}
        }
    }

    async fn forget(&mut self, path: &OwnedObjectPath) {
        let deleted = self
            .call::<()>(path.as_str(), CONNECTION, "Delete", &())
            .await;
        if let Err(error) = deleted {
            eprintln!("block-app: NetworkManager did not delete a Wi-Fi connection: {error}");
        }
        self.saved_stale = true;
    }

    async fn refresh(&mut self, report: &mut impl FnMut(WifiStatus)) {
        if self.saved_stale {
            self.saved_stale = false;
            match self.read_saved().await {
                Ok(saved) => self.saved = saved,
                Err(_) => self.saved = Saved::default(),
            }
        }
        let status = match self.read().await {
            Ok(status) => {
                self.unreadable = false;
                status
            }
            Err(error) => {
                if !std::mem::replace(&mut self.unreadable, true) {
                    eprintln!("block-app: NetworkManager's Wi-Fi cannot be read: {error}");
                }
                self.device = None;
                self.paths.clear();
                self.points.clear();
                WifiStatus::default()
            }
        };
        if self.reported.as_ref() != Some(&status) {
            self.reported = Some(status.clone());
            report(status);
        }
    }

    async fn read_saved(&self) -> zbus::Result<Saved> {
        let listed: Vec<OwnedObjectPath> = self
            .call(SETTINGS_PATH, SETTINGS, "ListConnections", &())
            .await?;
        let mut saved = Saved::default();
        for path in listed {
            let settings: zbus::Result<Settings> = self
                .call(path.as_str(), CONNECTION, "GetSettings", &())
                .await;
            if let Some(ssid) = settings.ok().as_ref().and_then(saved_ssid) {
                saved.paths.push(path);
                saved.ssids.push(ssid);
            }
        }
        Ok(saved)
    }

    async fn read(&mut self) -> zbus::Result<WifiStatus> {
        let manager = self.properties(PATH, MANAGER).await?;
        let enabled = flag(&manager, "WirelessEnabled");
        let devices: Vec<OwnedObjectPath> = self.call(PATH, MANAGER, "GetDevices", &()).await?;
        let mut device = None;
        for path in devices {
            let kind = self
                .properties(path.as_str(), DEVICE)
                .await
                .map(|properties| number(&properties, "DeviceType"));
            if kind.is_ok_and(|kind| kind == DEVICE_TYPE_WIFI) {
                device = Some(path);
                break;
            }
        }
        self.device = device.clone();
        let Some(device) = device else {
            self.paths.clear();
            self.points.clear();
            return Ok(WifiStatus {
                enabled,
                ..WifiStatus::default()
            });
        };
        let properties = self.properties(device.as_str(), DEVICE).await?;
        let state = number(&properties, "State");
        let wireless = self.properties(device.as_str(), WIRELESS).await?;
        let active_point = object(&wireless, "ActiveAccessPoint");
        let paths: Vec<OwnedObjectPath> = self
            .call(device.as_str(), WIRELESS, "GetAllAccessPoints", &())
            .await?;
        let mut points = Vec::new();
        let mut kept = Vec::new();
        for path in paths {
            if let Ok(properties) = self.properties(path.as_str(), ACCESS_POINT).await {
                points.push(access_point(&properties));
                kept.push(path);
            }
        }
        self.points = points;
        self.paths = kept;
        let mut active = None;
        if let Some(connection) = object(&properties, "ActiveConnection")
            && let Ok(active_properties) = self.properties(connection.as_str(), ACTIVE).await
            && let Some(saved) = object(&active_properties, "Connection")
            && let Some(index) = self.saved.paths.iter().position(|path| *path == saved)
        {
            active = self.saved.ssids.get(index).cloned();
        }
        if active.is_none()
            && let Some(point) = active_point
            && let Some(index) = self.paths.iter().position(|path| *path == point)
        {
            active = self.points.get(index).map(|point| point.ssid.clone());
        }
        let active = active.map(|ssid| String::from_utf8_lossy(&ssid).into_owned());
        self.settle(state, active.as_deref()).await;
        let link = match (model::link(state, active), self.attempts.trying()) {
            (WifiLink::Connected(ssid), _) => WifiLink::Connected(ssid),
            (_, Some(trying)) => WifiLink::Connecting(trying.to_owned()),
            (link, None) => link,
        };
        Ok(WifiStatus {
            available: true,
            enabled,
            link,
            networks: match enabled {
                true => model::networks(&self.points, &self.saved.ssids),
                false => Vec::new(),
            },
            failed: self.attempts.failed.clone(),
        })
    }

    async fn request(&mut self, request: WifiRequest) {
        match request {
            WifiRequest::SetEnabled(enabled) => {
                let set = self
                    .call::<()>(
                        PATH,
                        PROPERTIES,
                        "Set",
                        &(MANAGER, "WirelessEnabled", Value::from(enabled)),
                    )
                    .await;
                if let Err(error) = set {
                    eprintln!("block-app: NetworkManager did not turn Wi-Fi on or off: {error}");
                }
            }
            WifiRequest::Scan => {
                if let Some(device) = &self.device {
                    let options: HashMap<&str, Value<'_>> = HashMap::new();
                    let _ = self
                        .call::<()>(device.as_str(), WIRELESS, "RequestScan", &(options,))
                        .await;
                }
            }
            WifiRequest::Disconnect => {
                self.attempts.cancel();
                if let Some(device) = &self.device {
                    let left = self
                        .call::<()>(device.as_str(), DEVICE, "Disconnect", &())
                        .await;
                    if let Err(error) = left {
                        eprintln!("block-app: NetworkManager did not disconnect: {error}");
                    }
                }
            }
            WifiRequest::Connect { ssid, password } => self.connect(ssid, password).await,
        }
    }

    async fn connect(&mut self, ssid: String, password: Option<String>) {
        if let Some(path) = self.added.take() {
            self.forget(&path).await;
        }
        self.replaces.clear();
        let Some(device) = self.device.clone() else {
            self.attempts.refused(ssid);
            return;
        };
        let plan = model::plan(&ssid, password, &self.points, &self.saved.ssids);
        let point = |index: Option<usize>| {
            index
                .and_then(|index| self.paths.get(index).cloned())
                .unwrap_or_else(|| ObjectPath::from_static_str_unchecked(NO_OBJECT).into())
        };
        let started = match plan {
            Plan::Refuse => Err(None),
            Plan::Activate {
                connection,
                point: index,
            } => {
                let Some(connection) = self.saved.paths.get(connection).cloned() else {
                    self.attempts.refused(ssid);
                    return;
                };
                let specific = point(index);
                self.attempts.start(ssid.clone());
                self.call::<OwnedObjectPath>(
                    PATH,
                    MANAGER,
                    "ActivateConnection",
                    &(connection, device, specific),
                )
                .await
                .map(|_| ())
                .map_err(Some)
            }
            Plan::Add {
                ssid: bytes,
                key_management,
                password,
                point: index,
                replaces,
            } => {
                let specific = point(index);
                let replaced: Vec<OwnedObjectPath> = replaces
                    .iter()
                    .filter_map(|index| self.saved.paths.get(*index).cloned())
                    .collect();
                let settings = added_settings(bytes, key_management, password);
                self.attempts.start(ssid.clone());
                let added = self
                    .call::<(OwnedObjectPath, OwnedObjectPath)>(
                        PATH,
                        MANAGER,
                        "AddAndActivateConnection",
                        &(settings, device, specific),
                    )
                    .await;
                added
                    .map(|(path, _)| {
                        self.added = Some(path);
                        self.replaces = replaced;
                    })
                    .map_err(Some)
            }
        };
        if let Err(error) = started {
            if let Some(error) = error {
                eprintln!("block-app: NetworkManager did not connect to {ssid:?}: {error}");
            }
            self.attempts.refused(ssid);
        }
    }
}
