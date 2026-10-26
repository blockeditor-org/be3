use block_plugin_api::{WifiLink, WifiNetwork, WifiSecurity};

const PRIVACY: u32 = 0x1;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_802_1X: u32 = 0x200;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

pub(crate) const DISCONNECTED: u32 = 30;
pub(crate) const PREPARE: u32 = 40;
pub(crate) const SECONDARIES: u32 = 90;
pub(crate) const ACTIVATED: u32 = 100;
pub(crate) const FAILED: u32 = 120;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AccessPoint {
    pub(crate) ssid: Vec<u8>,
    pub(crate) strength: u8,
    pub(crate) flags: u32,
    pub(crate) wpa: u32,
    pub(crate) rsn: u32,
}

impl AccessPoint {
    pub(crate) fn name(&self) -> String {
        String::from_utf8_lossy(&self.ssid).into_owned()
    }

    pub(crate) fn security(&self) -> WifiSecurity {
        let managed = self.wpa | self.rsn;
        if managed & (KEY_MGMT_802_1X | KEY_MGMT_EAP_SUITE_B_192) != 0 {
            return WifiSecurity::Unsupported;
        }
        if managed & (KEY_MGMT_PSK | KEY_MGMT_SAE) != 0 {
            return WifiSecurity::Password;
        }
        if managed & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 {
            return WifiSecurity::Open;
        }
        match self.flags & PRIVACY != 0 {
            true => WifiSecurity::Unsupported,
            false => WifiSecurity::Open,
        }
    }

    pub(crate) fn key_management(&self) -> Option<&'static str> {
        let managed = self.wpa | self.rsn;
        match self.security() {
            WifiSecurity::Password if managed & KEY_MGMT_PSK != 0 => Some("wpa-psk"),
            WifiSecurity::Password => Some("sae"),
            WifiSecurity::Open if managed & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 => Some("owe"),
            _ => None,
        }
    }
}

pub(crate) fn strongest(points: &[AccessPoint], name: &str) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .filter(|(_, point)| !point.ssid.is_empty() && point.name() == name)
        .max_by_key(|(_, point)| point.strength)
        .map(|(index, _)| index)
}

pub(crate) fn networks(points: &[AccessPoint], saved: &[Vec<u8>]) -> Vec<WifiNetwork> {
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

pub(crate) fn link(state: u32, ssid: Option<String>) -> WifiLink {
    match (state, ssid) {
        (ACTIVATED, Some(ssid)) => WifiLink::Connected(ssid),
        (PREPARE..=SECONDARIES, Some(ssid)) => WifiLink::Connecting(ssid),
        _ => WifiLink::Disconnected,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Activate {
        connection: usize,
        point: Option<usize>,
    },
    Add {
        ssid: Vec<u8>,
        key_management: Option<&'static str>,
        password: Option<String>,
        point: Option<usize>,
        replaces: Vec<usize>,
    },
    Refuse,
}

pub(crate) fn plan(
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
    match (found.security(), password) {
        (WifiSecurity::Open, _) => Plan::Add {
            ssid,
            key_management: found.key_management(),
            password: None,
            point,
            replaces: same,
        },
        (WifiSecurity::Password, Some(password)) => Plan::Add {
            ssid,
            key_management: found.key_management(),
            password: Some(password),
            point,
            replaces: same,
        },
        _ => Plan::Refuse,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Connected,
    Failed,
}

#[derive(Debug, Default)]
pub(crate) struct Attempts {
    trying: Option<String>,
    began: bool,
    pub(crate) failed: Option<String>,
}

impl Attempts {
    pub(crate) fn start(&mut self, ssid: String) {
        self.trying = Some(ssid);
        self.began = false;
        self.failed = None;
    }

    pub(crate) fn trying(&self) -> Option<&str> {
        self.trying.as_deref()
    }

    pub(crate) fn refused(&mut self, ssid: String) {
        self.trying = None;
        self.failed = Some(ssid);
    }

    pub(crate) fn cancel(&mut self) {
        self.trying = None;
        self.failed = None;
    }

    pub(crate) fn state(&mut self, state: u32, active: Option<&str>) -> Option<Outcome> {
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
