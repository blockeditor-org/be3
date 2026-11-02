use be_sdk_api::{WifiLink, WifiNetwork, WifiSecurity};

const PRIVACY: u32 = 0x1;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_802_1X: u32 = 0x200;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

pub const DISCONNECTED: u32 = 30;
pub const PREPARE: u32 = 40;
pub const SECONDARIES: u32 = 90;
pub const ACTIVATED: u32 = 100;
pub const FAILED: u32 = 120;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessPoint {
    pub ssid: Vec<u8>,
    pub strength: u8,
    pub flags: u32,
    pub wpa: u32,
    pub rsn: u32,
}

impl AccessPoint {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.ssid).into_owned()
    }

    pub fn security(&self) -> WifiSecurity {
        let managed = self.wpa | self.rsn;
        if managed & (KEY_MGMT_802_1X | KEY_MGMT_EAP_SUITE_B_192) != 0 {
            return WifiSecurity::Enterprise;
        }
        if managed & (KEY_MGMT_PSK | KEY_MGMT_SAE) != 0 {
            return WifiSecurity::Password;
        }
        if managed & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 {
            return WifiSecurity::Open;
        }
        match self.flags & PRIVACY != 0 {
            true => WifiSecurity::Wep,
            false => WifiSecurity::Open,
        }
    }

    pub fn key(&self, password: Option<String>) -> Option<Key> {
        let managed = self.wpa | self.rsn;
        match (self.security(), password) {
            (WifiSecurity::Open, _) if managed & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 => {
                Some(Key::Owe)
            }
            (WifiSecurity::Open, _) => Some(Key::Open),
            (WifiSecurity::Password, Some(password)) if passphrase(&password) => {
                match managed & KEY_MGMT_PSK != 0 {
                    true => Some(Key::Psk(password)),
                    false => Some(Key::Sae(password)),
                }
            }
            (WifiSecurity::Wep, Some(key)) if wep_key(&key) => Some(Key::Wep(key)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Open,
    Owe,
    Psk(String),
    Sae(String),
    Wep(String),
}

impl Key {
    pub fn hidden(security: WifiSecurity, password: Option<String>) -> Option<Key> {
        match (security, password) {
            (WifiSecurity::Open, None) => Some(Key::Open),
            (WifiSecurity::Password, Some(password)) if passphrase(&password) => {
                Some(Key::Psk(password))
            }
            (WifiSecurity::Wep, Some(key)) if wep_key(&key) => Some(Key::Wep(key)),
            _ => None,
        }
    }
}

const PASSPHRASE_BYTES: std::ops::RangeInclusive<usize> = 8..=63;
const HEX_KEY_LENGTH: usize = 64;
const WEP_TEXT_LENGTHS: [usize; 2] = [5, 13];
const WEP_HEX_LENGTHS: [usize; 2] = [10, 26];

pub fn passphrase(password: &str) -> bool {
    let printable = password.chars().all(|letter| !letter.is_control());
    let hex = password.len() == HEX_KEY_LENGTH
        && password.chars().all(|letter| letter.is_ascii_hexdigit());
    printable && (PASSPHRASE_BYTES.contains(&password.len()) || hex)
}

pub fn wep_key(key: &str) -> bool {
    let text = WEP_TEXT_LENGTHS.contains(&key.len())
        && key
            .chars()
            .all(|letter| letter.is_ascii() && !letter.is_control());
    let hex = WEP_HEX_LENGTHS.contains(&key.len())
        && key.chars().all(|letter| letter.is_ascii_hexdigit());
    text || hex
}

pub fn name_hidden(points: &mut [AccessPoint], active: Option<usize>, ssid: Option<&[u8]>) {
    if let Some(point) = active.and_then(|index| points.get_mut(index))
        && point.ssid.is_empty()
        && let Some(ssid) = ssid
    {
        point.ssid = ssid.to_vec();
    }
}

pub fn strongest(points: &[AccessPoint], name: &str) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .filter(|(_, point)| !point.ssid.is_empty() && point.name() == name)
        .max_by_key(|(_, point)| point.strength)
        .map(|(index, _)| index)
}

pub fn networks(points: &[AccessPoint], saved: &[Vec<u8>]) -> Vec<WifiNetwork> {
    let mut listed: Vec<WifiNetwork> = Vec::new();
    for point in points.iter().filter(|point| !point.ssid.is_empty()) {
        let ssid = point.name();
        if listed.iter().any(|network| network.ssid == ssid) {
            continue;
        }
        let Some(best) = strongest(points, &ssid).and_then(|index| points.get(index)) else {
            continue;
        };
        listed.push(WifiNetwork {
            ssid,
            strength: best.strength.min(100),
            security: best.security(),
            saved: saved.contains(&best.ssid),
        });
    }
    listed.sort_by(|one, other| {
        other
            .strength
            .cmp(&one.strength)
            .then_with(|| one.ssid.cmp(&other.ssid))
    });
    listed
}

pub fn link(state: u32, ssid: Option<String>) -> WifiLink {
    match (state, ssid) {
        (ACTIVATED, Some(ssid)) => WifiLink::Connected(ssid),
        (PREPARE..=SECONDARIES, Some(ssid)) => WifiLink::Connecting(ssid),
        _ => WifiLink::Disconnected,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    Activate {
        connection: usize,
        point: Option<usize>,
    },
    Add {
        ssid: Vec<u8>,
        key: Key,
        hidden: bool,
        point: Option<usize>,
        replaces: Vec<usize>,
    },
    Refuse,
}

pub fn plan(
    name: &str,
    password: Option<String>,
    points: &[AccessPoint],
    saved: &[Vec<u8>],
) -> Plan {
    let point = strongest(points, name);
    let found = point.and_then(|index| points.get(index));
    let ssid = match found {
        Some(found) => found.ssid.clone(),
        None => match saved
            .iter()
            .find(|bytes| String::from_utf8_lossy(bytes) == name)
        {
            Some(bytes) => bytes.clone(),
            None => return Plan::Refuse,
        },
    };
    let same: Vec<usize> = saved
        .iter()
        .enumerate()
        .filter(|(_, bytes)| **bytes == ssid)
        .map(|(index, _)| index)
        .collect();
    if password.is_none()
        && let Some(connection) = same.first()
    {
        return Plan::Activate {
            connection: *connection,
            point,
        };
    }
    let Some(found) = found else {
        return Plan::Refuse;
    };
    match found.key(password) {
        Some(key) => Plan::Add {
            ssid,
            key,
            hidden: false,
            point,
            replaces: same,
        },
        None => Plan::Refuse,
    }
}

pub fn plan_hidden(
    name: &str,
    security: WifiSecurity,
    password: Option<String>,
    saved: &[Vec<u8>],
) -> Plan {
    let ssid = name.as_bytes().to_vec();
    let Some(key) = Key::hidden(security, password) else {
        return Plan::Refuse;
    };
    let replaces = saved
        .iter()
        .enumerate()
        .filter(|(_, bytes)| **bytes == ssid)
        .map(|(index, _)| index)
        .collect();
    Plan::Add {
        ssid,
        key,
        hidden: true,
        point: None,
        replaces,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Connected,
    Failed,
}

#[derive(Debug, Default)]
pub struct Attempts {
    trying: Option<String>,
    began: bool,
    pub failed: Option<String>,
}

impl Attempts {
    pub fn start(&mut self, ssid: String) {
        self.trying = Some(ssid);
        self.began = false;
        self.failed = None;
    }

    pub fn trying(&self) -> Option<&str> {
        self.trying.as_deref()
    }

    pub fn refused(&mut self, ssid: String) {
        self.trying = None;
        self.failed = Some(ssid);
    }

    pub fn cancel(&mut self) {
        self.trying = None;
        self.failed = None;
    }

    pub fn state(&mut self, state: u32, active: Option<&str>) -> Option<Outcome> {
        let trying = self.trying.as_deref()?;
        match state {
            PREPARE..=SECONDARIES => {
                self.began = true;
                None
            }
            ACTIVATED if self.began || active == Some(trying) => {
                self.trying = None;
                Some(Outcome::Connected)
            }
            FAILED | DISCONNECTED if self.began => {
                self.failed = self.trying.take();
                Some(Outcome::Failed)
            }
            _ => None,
        }
    }
}
